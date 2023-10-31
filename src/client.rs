use crate::generated::svn_client_version;
use crate::Version;

pub fn version() -> Version {
    unsafe {
        let version = svn_client_version();
        Version(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = version();
        assert_eq!(version.major(), 1);
    }
}
