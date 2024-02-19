use radicle::cob::{ObjectId, Timestamp};
use radicle::prelude::Did;

/// Format a git Oid.
pub fn oid(oid: impl Into<radicle::git::Oid>) -> String {
    format!("{:.7}", oid.into())
}

/// Format a COB id.
pub fn cob(id: &ObjectId) -> String {
    format!("{:.7}", id.to_string())
}

/// Format a DID.
pub fn did(did: &Did) -> String {
    let nid = did.as_key().to_human();
    format!("{}…{}", &nid[..7], &nid[nid.len() - 7..])
}

/// Format a timestamp.
pub fn timestamp(time: &Timestamp) -> String {
    let fmt = timeago::Formatter::new();
    let now = Timestamp::now();
    let duration = std::time::Duration::from_secs(now.as_secs() - time.as_secs());

    fmt.convert(duration)
}
