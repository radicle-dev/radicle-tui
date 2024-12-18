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
    use std::path::Path;

    use anyhow::Result;

    use radicle::cob::cache::NoCache;
    use radicle::crypto::{Signer, Verified};
    use radicle::git;
    use radicle::identity::{RepoId, Visibility};
    use radicle::patch::{Cache, MergeTarget, PatchMut, Patches};
    use radicle::rad;
    use radicle::storage::git::Repository;
    use radicle::storage::refs::SignedRefs;
    use radicle::storage::ReadStorage;
    use radicle::test::setup::{BranchWith, Node};
    use radicle::Storage;

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
    pub fn project<P: AsRef<Path>, G: Signer>(
        path: P,
        storage: &Storage,
        signer: &G,
    ) -> Result<(RepoId, SignedRefs<Verified>, git2::Repository, git2::Oid), rad::InitError> {
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
}
