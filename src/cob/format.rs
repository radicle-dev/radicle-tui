use radicle::identity::Did;

/// Format a DID.
pub fn did(did: &Did) -> String {
    let nid = did.as_key().to_human();
    format!("{}…{}", &nid[..7], &nid[nid.len() - 7..])
}
