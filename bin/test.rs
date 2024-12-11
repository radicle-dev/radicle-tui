pub mod fixtures {
    use anyhow::Result;

    use radicle::cob::cache::NoCache;
    use radicle::patch::{Cache, MergeTarget, PatchMut, Patches};
    use radicle::storage::git::Repository;
    use radicle::test::setup::{BranchWith, NodeWithRepo};

    pub fn node_with_repo() -> NodeWithRepo {
        NodeWithRepo::default()
    }

    pub fn branch_with_eof_removed(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("README", b"Hello World!")])
    }

    pub fn branch_with_line_added(node: &NodeWithRepo) -> BranchWith {
        let checkout = node.repo.checkout();
        checkout.branch_with([("README", b"Hello World!\nHello World!\n")])
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
