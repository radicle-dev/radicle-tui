pub mod fixtures {
    use std::path::Path;

    use anyhow::Result;

    use radicle::cob::cache::NoCache;
    use radicle::git;
    use radicle::patch::{Cache, MergeTarget, PatchMut, Patches};
    use radicle::storage::git::Repository;
    use radicle::storage::WriteRepository;
    use radicle::test::setup::{BranchWith, NodeWithRepo};

    /// The birth of the radicle project, January 1st, 2018.
    pub const RADICLE_EPOCH: i64 = 1514817556;

    pub fn node_with_repo() -> NodeWithRepo {
        let node = NodeWithRepo::default();
        let repo = node.repo.repo.raw();

        let sig = git2::Signature::new(
            "anonymous",
            "anonymous@radicle.xyz",
            &git2::Time::new(RADICLE_EPOCH, 0),
        )
        .unwrap();

        let head = repo
            .find_commit(repo.head().unwrap().target().unwrap())
            .unwrap();
        let tree =
            git::write_tree(Path::new("main.rs"), "pub fn main() {}\n".as_bytes(), &repo).unwrap();

        git::commit(
            &repo,
            &head,
            git::refname!("refs/heads/master").as_refstr(),
            "Another commit",
            &sig,
            &tree,
        )
        .unwrap();

        drop(tree);
        drop(head);

        node
    }

    pub fn branch_with_eof_removed(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("README", b"Hello World!")])
    }

    pub fn branch_with_line_added(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("README", b"Hello World!\nHello World!\n")])
    }

    pub fn branch_with_files_added(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("CONTRIBUTE", b"TBD\n"), ("LICENSE", b"TBD\n")])
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
}
