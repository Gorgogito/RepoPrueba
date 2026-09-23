pub mod branches;
pub mod changes;
pub mod remotes;
pub mod repo;
#[cfg(test)]
mod test_support;

pub(crate) fn err_msg(e: git2::Error) -> String {
    e.message().to_string()
}
