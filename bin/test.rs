pub mod setup {

    use std::path::Path;

    use radicle::git;
    use radicle::rad;
    use radicle::storage::git::Repository;
    use radicle::test::setup::{BranchWith, Node};

    /// A node with a repository.
    pub struct NodeWithRepo {
        pub node: Node,
        pub repo: NodeRepo,
    }

    impl std::ops::Deref for NodeWithRepo {
        type Target = Node;

        fn deref(&self) -> &Self::Target {
            &self.node
        }
    }

    impl std::ops::DerefMut for NodeWithRepo {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.node
        }
    }

    /// A node repository with an optional checkout.
    pub struct NodeRepo {
        pub repo: Repository,
        pub checkout: Option<NodeRepoCheckout>,
    }

    impl NodeRepo {
        #[track_caller]
        pub fn checkout(&self) -> &NodeRepoCheckout {
            self.checkout.as_ref().unwrap()
        }
    }

    impl std::ops::Deref for NodeRepo {
        type Target = Repository;

        fn deref(&self) -> &Self::Target {
            &self.repo
        }
    }

    impl std::ops::DerefMut for NodeRepo {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.repo
        }
    }

    /// A repository checkout.
    pub struct NodeRepoCheckout {
        pub checkout: git::raw::Repository,
    }

    impl NodeRepoCheckout {
        pub fn branch_with<S: AsRef<Path>, T: AsRef<[u8]>>(
            &self,
            blobs: impl IntoIterator<Item = (S, T)>,
        ) -> BranchWith {
            let refname = git::Qualified::from(git::lit::refs_heads(git::refname!("master")));
            let base = self.checkout.refname_to_id(refname.as_str()).unwrap();
            let parent = self.checkout.find_commit(base).unwrap();
            let oid = commit(&self.checkout, &refname, blobs, &[&parent]);

            git::push(&self.checkout, &rad::REMOTE_NAME, [(&refname, &refname)]).unwrap();

            BranchWith {
                base: base.into(),
                oid,
            }
        }
    }

    impl std::ops::Deref for NodeRepoCheckout {
        type Target = git::raw::Repository;

        fn deref(&self) -> &Self::Target {
            &self.checkout
        }
    }

    pub fn commit<S: AsRef<Path>, T: AsRef<[u8]>>(
        repo: &git2::Repository,
        refname: &git::Qualified,
        blobs: impl IntoIterator<Item = (S, T)>,
        parents: &[&git2::Commit<'_>],
    ) -> git::Oid {
        let tree = {
            let mut tb = repo.treebuilder(None).unwrap();
            for (name, blob) in blobs.into_iter() {
                let oid = repo.blob(blob.as_ref()).unwrap();
                tb.insert(name.as_ref(), oid, git2::FileMode::Blob.into())
                    .unwrap();
            }
            tb.write().unwrap()
        };
        let tree = repo.find_tree(tree).unwrap();
        let author = git2::Signature::now("anonymous", "anonymous@example.com").unwrap();

        repo.commit(
            Some(refname.as_str()),
            &author,
            &author,
            "Making changes",
            &tree,
            parents,
        )
        .unwrap()
        .into()
    }
}

pub mod fixtures {
    use std::path::{Path, PathBuf};

    use anyhow::Result;

    use radicle::cob::cache::NoCache;
    use radicle::crypto;
    use radicle::crypto::Verified;
    use radicle::git;
    use radicle::identity::{RepoId, Visibility};
    use radicle::node::device::Device;
    use radicle::patch::{Cache, MergeTarget, PatchMut, Patches};
    use radicle::rad;
    use radicle::storage::git::Repository;
    use radicle::storage::refs::SignedRefs;
    use radicle::storage::ReadStorage;
    use radicle::test::setup::{BranchWith, Node};
    use radicle::Storage;
    use radicle_cli::git::unified_diff::FileHeader;
    use radicle_git_ext::Oid;
    use radicle_surf::diff::{self, DiffFile, Hunk, Line, Modification};

    use crate::git::HunkDiff;

    use super::setup::{NodeRepo, NodeRepoCheckout, NodeWithRepo};

    /// The birth of the radicle project, January 1st, 2018.
    pub const RADICLE_EPOCH: i64 = 1514817556;
    pub const MAIN_RS: &str = r#"// This is a comment, and is ignored by the compiler.
// You can test this code by clicking the "Run" button over there ->
// or if you prefer to use your keyboard, you can use the "Ctrl + Enter"
// shortcut.

// This code is editable, feel free to hack it!
// You can always return to the original code by clicking the "Reset" button ->

// This is the main function.
fn main() {
    // Statements here are executed when the compiled binary is called.

    // Print text to the console.
    println!("Hello World!");
}
"#;

    pub fn node_with_repo() -> NodeWithRepo {
        let node = Node::default();
        let (id, _, checkout, _) =
            project(node.root.join("working"), &node.storage, &node.signer).unwrap();
        let repo = node.storage.repository(id).unwrap();
        let checkout = Some(NodeRepoCheckout { checkout });

        NodeWithRepo {
            node,
            repo: NodeRepo { repo, checkout },
        }
    }

    pub fn branch_with_eof_removed(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("README", b"Hello World!")])
    }

    pub fn branch_with_main_changed(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        let main_rs = r#"// This is a comment, and is ignored by the compiler.
// You can test this code by clicking the "Run" button over there ->
// or if you prefer to use your keyboard, you can use the "Ctrl + Enter"
// shortcut.

// This is a new comment.

// This code is editable, feel free to hack it!
// You can always return to the original code by clicking the "Reset" button ->

// This is the main function.
fn main() {
    // Statements here are executed when the compiled binary is called.

    // Print text to the console.
    println!("Hello World!");
    println!("Hello again");
}
"#;

        checkout.branch_with([("main.rs", main_rs.as_bytes())])
    }

    pub fn branch_with_main_emptied(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("main.rs", b"")])
    }

    pub fn branch_with_main_deleted_and_file_added(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("CONTRIBUTE", b"TBD\n")])
    }

    pub fn patch<'a, 'g>(
        node: &'a NodeWithRepo,
        branch: &BranchWith,
        patches: &'a mut Cache<Patches<'a, Repository>, NoCache>,
    ) -> Result<PatchMut<'a, 'g, Repository, NoCache>> {
        let patch = patches.create(
            "My first patch",
            "Blah blah blah.",
            MergeTarget::Delegates,
            branch.base,
            branch.oid,
            &[],
            &node.signer,
        )?;

        Ok(patch)
    }

    /// Create a new repository at the given path, and initialize it into a project.
    pub fn project<P: AsRef<Path>, G>(
        path: P,
        storage: &Storage,
        signer: &Device<G>,
    ) -> Result<(RepoId, SignedRefs<Verified>, git2::Repository, git2::Oid), rad::InitError>
    where
        G: crypto::signature::Signer<crypto::Signature>,
    {
        radicle::storage::git::transport::local::register(storage.clone());

        let (working, head) = repository(path);
        let (id, _, refs) = rad::init(
            &working,
            "acme".try_into().unwrap(),
            "Acme's repository",
            git::refname!("master"),
            Visibility::default(),
            signer,
            storage,
        )?;

        Ok((id, refs, working, head))
    }

    /// Creates a regular repository at the given path with a couple of commits.
    pub fn repository<P: AsRef<Path>>(path: P) -> (git2::Repository, git2::Oid) {
        let repo = git2::Repository::init(path).unwrap();
        let sig = git2::Signature::new(
            "anonymous",
            "anonymous@radicle.xyz",
            &git2::Time::new(RADICLE_EPOCH, 0),
        )
        .unwrap();
        let head = git::initial_commit(&repo, &sig).unwrap();
        let tree = git::write_tree(Path::new("main.rs"), MAIN_RS.as_bytes(), &repo).unwrap();
        let oid = {
            let commit = git::commit(
                &repo,
                &head,
                git::refname!("refs/heads/master").as_refstr(),
                "Second commit",
                &sig,
                &tree,
            )
            .unwrap();

            commit.id()
        };
        repo.set_head("refs/heads/master").unwrap();
        repo.checkout_head(None).unwrap();

        drop(tree);
        drop(head);

        (repo, oid)
    }

    /// @@ -3,8 +3,7 @@
    /// 3   3     // or if you prefer to use your keyboard, you can use the "Ctrl + Enter"
    /// 4   4     // shortcut.
    /// 5   5
    /// 6       - // This code is editable, feel free to hack it!
    /// 7       - // You can always return to the original code by clicking the "Reset" button ->
    ///     6   + // This is still a comment.
    /// 8   7
    /// 9   8     // This is the main function.
    /// 10  9     fn main() {
    pub fn simple_modified_hunk_diff(path: &PathBuf, commit: Oid) -> Result<HunkDiff> {
        let diff = DiffFile {
            oid: commit,
            mode: diff::FileMode::Blob,
        };

        Ok(HunkDiff::Modified {
            path: path.clone(),
            header: FileHeader::Modified {
                path: path.to_path_buf(),
                old: diff.clone(),
                new: diff.clone(),
                binary: false,
            },
            old: diff.clone(),
            new: diff,
            hunk: Some(Hunk {
                header: Line::from(b"@@ -3,8 +3,7 @@\n".to_vec()),
                lines: vec![
                    Modification::context(
                        b"// or if you prefer to use your keyboard, you can use the \"Ctrl + Enter\"\n"
                            .to_vec(),
                        3,
                        3,
                    ),
                    Modification::context(b"// shortcut.\n".to_vec(), 4, 4),
                    Modification::context(b"\n".to_vec(), 5, 5),
                    Modification::deletion(
                        b"// This code is editable, feel free to hack it!\n".to_vec(),
                        6,
                    ),
                    Modification::deletion(
                        b"// You can always return to the original code by clicking the \"Reset\" button ->\n".to_vec(),
                        7,
                    ),
                    Modification::addition(b"// This is still a comment.\n".to_vec(), 6),
                    Modification::context(b"\n".to_vec(), 8, 7),
                    Modification::context(b"// This is the main function.\n".to_vec(), 9, 8),
                    Modification::context(b"fn main() {\n".to_vec(), 10, 9),
                ],
                old: 3..11,
                new: 3..10,
            }),
            _stats: None,
        })
    }

    /// @@ -1,17 +1,15 @@
    /// 1       - use radicle::issue::IssueId;
    /// 2       - use tui::ui::state::ItemState;
    /// 3       - use tui::SelectionExit;
    /// 4   1     use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};
    /// 5   2     use tuirealm::event::{Event, Key, KeyEvent};
    /// 6   3     use tuirealm::{MockComponent, NoUserEvent};
    /// 7   4
    /// 8   5     use radicle_tui as tui;
    /// 9   6
    ///     7   + use tui::ui::state::ItemState;
    /// 10  8     use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer};
    /// 11  9     use tui::ui::widget::context::{ContextBar, Shortcuts};
    /// 12  10    use tui::ui::widget::list::PropertyList;
    /// 13      -
    /// 14  11    use tui::ui::widget::Widget;
    ///     12  + use tui::{Id, SelectionExit};
    /// 15  13
    /// 16  14    use super::ui::{IdSelect, OperationSelect};
    /// 17  15    use super::{IssueOperation, Message};
    pub fn complex_modified_hunk_diff(path: &PathBuf, commit: Oid) -> Result<HunkDiff> {
        let diff = DiffFile {
            oid: commit,
            mode: diff::FileMode::Blob,
        };

        Ok(HunkDiff::Modified {
            path: path.clone(),
            header: FileHeader::Modified {
                path: path.to_path_buf(),
                old: diff.clone(),
                new: diff.clone(),
                binary: false,
            },
            old: diff.clone(),
            new: diff,
            hunk: Some(Hunk {
                header: Line::from(b"@@ -1,17 +1,15 @@\n".to_vec()),
                lines: vec![
                    Modification::deletion(b"use radicle::issue::IssueId;\n".to_vec(), 1),
                    Modification::deletion(b"use tui::ui::state::ItemState;\n".to_vec(), 2),
                    Modification::deletion(b"use tui::SelectionExit;\n".to_vec(), 3),
                    Modification::context(
                        b"use tuirealm::command::{Cmd, CmdResult, Direction as MoveDirection};\n"
                            .to_vec(),
                        4,
                        1,
                    ),
                    Modification::context(
                        b"use tuirealm::event::{Event, Key, KeyEvent};\n".to_vec(),
                        5,
                        2,
                    ),
                    Modification::context(
                        b"use tuirealm::{MockComponent, NoUserEvent};\n".to_vec(),
                        6,
                        3,
                    ),
                    Modification::context(b"\n".to_vec(), 7, 4),
                    Modification::context(b"use radicle_tui as tui;\n".to_vec(), 8, 5),
                    Modification::context(b"\n".to_vec(), 9, 6),
                    Modification::addition(b"use tui::ui::state::ItemState;\n".to_vec(), 7),
                    Modification::context(b"use tui::ui::widget::container::{AppHeader, GlobalListener, LabeledContainer};\n"
                        .to_vec(),
                        10,
                        8,
                    ),
                    Modification::context(
                        b"use tui::ui::widget::context::{ContextBar, Shortcuts};\n".to_vec(),
                        11,
                        9,
                    ),
                    Modification::context(
                        b"use tui::ui::widget::list::PropertyList;\n".to_vec(),
                        12,
                        10,
                    ),
                    Modification::deletion(b"\n".to_vec(), 13),
                    Modification::context(b"use tui::ui::widget::Widget;\n".to_vec(), 14, 11),
                    Modification::addition(b"use tui::{Id, SelectionExit};\n".to_vec(), 12),
                    Modification::context(b"\n".to_vec(), 15, 13),
                    Modification::context(
                        b"use super::ui::{IdSelect, OperationSelect};\n".to_vec(),
                        16,
                        14,
                    ),
                    Modification::context(
                        b"use super::{IssueOperation, Message};\n".to_vec(),
                        17,
                        15,
                    ),
                ],
                old: 1..18,
                new: 1..16,
            }),
            _stats: None,
        })
    }

    /// @@ -1,1 +0,0 @@
    /// - TBD
    pub fn deleted_hunk_diff(path: &PathBuf, commit: Oid) -> Result<HunkDiff> {
        let diff = DiffFile {
            oid: commit,
            mode: diff::FileMode::Blob,
        };

        Ok(HunkDiff::Deleted {
            path: path.clone(),
            header: FileHeader::Deleted {
                path: path.to_path_buf(),
                old: diff.clone(),
                binary: false,
            },
            old: diff.clone(),
            hunk: Some(Hunk {
                header: Line::from(b"@@ -1,1 +0,0 @@\n".to_vec()),
                lines: vec![Modification::deletion(b"TBD\n".to_vec(), 1)],
                old: 1..2,
                new: 0..0,
            }),
            _stats: None,
        })
    }
}
