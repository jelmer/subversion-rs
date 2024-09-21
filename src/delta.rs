use crate::Revnum;
use apr::pool::Pool;

pub fn version() -> crate::Version {
    crate::Version(unsafe { crate::generated::svn_delta_version() })
}

pub struct WrapEditor(
    pub(crate) *const crate::generated::svn_delta_editor_t,
    pub(crate) apr::pool::PooledPtr<std::ffi::c_void>,
);

impl Editor for WrapEditor {
    fn set_target_revision(&mut self, revision: Revnum) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).set_target_revision.unwrap())(
                self.1.as_mut_ptr(),
                revision,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn open_root<'b>(
        &'b mut self,
        base_revision: Revnum,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*self.0).open_root.unwrap())(
                self.1.as_mut_ptr(),
                base_revision,
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(&self.0, editor)))
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).close_edit.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).abort_edit.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapDirectoryEditor<'a>(
    pub(crate) &'a *const crate::generated::svn_delta_editor_t,
    pub(crate) apr::pool::PooledPtr<std::ffi::c_void>,
);

impl<'a> DirectoryEditor for WrapDirectoryEditor<'a> {
    fn delete_entry(&mut self, path: &str, revision: Option<Revnum>) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).delete_entry.unwrap())(
                path.as_ptr() as *const i8,
                revision.map(|r| r).unwrap_or(0),
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_directory<'b>(
        &'b mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let copyfrom_path = copyfrom.map(|(p, _)| p);
            let copyfrom_rev = copyfrom.map(|(_, r)| r).unwrap_or(0);
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).add_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(self.0, editor)))
    }

    fn open_directory<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).open_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                base_revision.unwrap_or(0),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(self.0, editor)))
    }

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.0)).change_dir_prop.unwrap())(
                self.1.as_mut_ptr(),
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).close_directory.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).absent_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_file<'b>(
        &'b mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let copyfrom_path = copyfrom.map(|(p, _)| p);
            let copyfrom_rev = copyfrom.map(|(_, r)| r).unwrap_or(0);
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).add_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapFileEditor(self.0, editor)))
    }

    fn open_file<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).open_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                base_revision.map(|r| r.into()).unwrap_or(0),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapFileEditor(self.0, editor)))
    }

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).absent_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapFileEditor<'a>(
    &'a *const crate::generated::svn_delta_editor_t,
    apr::pool::PooledPtr<std::ffi::c_void>,
);

pub struct WrapTxdeltaWindowHandler(
    apr::pool::PooledPtr<crate::generated::svn_txdelta_window_handler_t>,
    apr::pool::PooledPtr<std::ffi::c_void>,
);

impl<'a> FileEditor for WrapFileEditor<'a> {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error>>, crate::Error>
    {
        let mut pool = self.1.pool().subpool();
        let mut handler = None;
        let mut baton = std::ptr::null_mut();
        let err = unsafe {
            ((*(*self.0)).apply_textdelta.unwrap())(
                self.1.as_mut_ptr(),
                if let Some(base_checksum) = base_checksum {
                    base_checksum.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
                &mut handler,
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        let apply = move |window: &mut TxDeltaWindow| -> Result<(), crate::Error> {
            let err = unsafe { (handler.unwrap())(window.as_mut_ptr(), baton) };
            crate::Error::from_raw(err)?;
            Ok(())
        };
        Ok(Box::new(apply))
    }

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.0)).change_file_prop.unwrap())(
                self.1.as_mut_ptr(),
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error> {
        let mut pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).close_file.unwrap())(
                self.1.as_mut_ptr(),
                if let Some(text_checksum) = text_checksum {
                    text_checksum.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub trait Editor {
    fn set_target_revision(&mut self, revision: Revnum) -> Result<(), crate::Error>;

    fn open_root<'a>(
        &'a mut self,
        base_revision: Revnum,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn close(&mut self) -> Result<(), crate::Error>;

    fn abort(&mut self) -> Result<(), crate::Error>;
}

pub trait DirectoryEditor {
    fn delete_entry(&mut self, path: &str, revision: Option<Revnum>) -> Result<(), crate::Error>;

    fn add_directory<'a>(
        &'a mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn open_directory<'a>(
        &'a mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error>;

    fn close(&mut self) -> Result<(), crate::Error>;

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error>;

    fn add_file<'a>(
        &'a mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn FileEditor + 'a>, crate::Error>;

    fn open_file<'a>(
        &'a mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn FileEditor + 'a>, crate::Error>;

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error>;
}

pub fn noop_window_handler(window: &mut TxDeltaWindow) -> Result<(), crate::Error> {
    let err = unsafe {
        crate::generated::svn_delta_noop_window_handler(window.as_mut_ptr(), std::ptr::null_mut())
    };
    crate::Error::from_raw(err)?;
    Ok(())
}

pub trait FileEditor {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error>>, crate::Error>;

    // TODO: fn apply_textdelta_stream(&mut self, base_checksum: Option<&str>) -> Result<&dyn TextDelta, crate::Error>;

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error>;

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error>;
}

pub struct TxDeltaWindow(apr::pool::PooledPtr<crate::generated::svn_txdelta_window_t>);

impl TxDeltaWindow {
    pub fn as_ptr(&self) -> *const crate::generated::svn_txdelta_window_t {
        self.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_txdelta_window_t {
        self.0.as_mut_ptr()
    }

    pub fn new() -> Self {
        Self(apr::pool::PooledPtr::initialize(|p| Ok::<_, crate::Error>(p.calloc())).unwrap())
    }

    pub fn sview_len(&self) -> apr::apr_size_t {
        self.0.sview_len
    }

    pub fn tview_len(&self) -> apr::apr_size_t {
        self.0.tview_len
    }

    pub fn sview_offset(&self) -> crate::FileSize {
        self.0.sview_offset
    }

    pub fn compose(a: &Self, b: &Self) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_txdelta_compose_windows(
                    a.0.as_ptr(),
                    b.0.as_ptr(),
                    pool.as_mut_ptr(),
                ))
            })
            .unwrap(),
        )
    }

    pub fn apply_instructions(
        &mut self,
        source: &mut [u8],
        target: &mut Vec<u8>,
    ) -> Result<(), crate::Error> {
        unsafe {
            target.resize(self.tview_len(), 0);
            let mut tlen = target.len() as apr::apr_size_t;
            crate::generated::svn_txdelta_apply_instructions(
                self.0.as_mut_ptr(),
                source.as_ptr() as *mut i8,
                target.as_mut_ptr() as *mut i8,
                &mut tlen,
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert_eq!(super::version().major(), 1);
    }
}
