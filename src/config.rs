pub struct Config<'pool>(apr::hash::Hash<'pool, &'pool str, *mut crate::generated::svn_config_t>);
