use std::ffi::{c_char, c_void, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("I/O error during trash operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("Target does not exist: {0}")]
    TargetNotFound(PathBuf),

    #[error("Native macOS Trash error: {0}")]
    NativeTrashError(String),
}

/// Abstraction for moving items to trash or an isolated testing sink.
pub trait TrashBackend: Send + Sync {
    /// Moves target path to trash and returns the resulting destination path.
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError>;

    /// Returns a human-readable name of the backend.
    fn name(&self) -> &'static str;
}

#[cfg(target_os = "macos")]
#[link(name = "objc", kind = "dylib")]
#[link(name = "Foundation", kind = "framework")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[cfg(target_os = "macos")]
extern "C" {
    fn objc_msgSend();
}

#[cfg(target_os = "macos")]
type MsgSendNoArg = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
#[cfg(target_os = "macos")]
type MsgSendOneArg = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void;
#[cfg(target_os = "macos")]
type MsgSendIdArg = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
#[cfg(target_os = "macos")]
type MsgSendTrash = unsafe extern "C" fn(
    *mut c_void,
    *mut c_void,
    *mut c_void,
    *mut *mut c_void,
    *mut *mut c_void,
) -> i8;
#[cfg(target_os = "macos")]
type MsgSendCStr = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_char;

#[cfg(target_os = "macos")]
fn native_macos_trash(target: &Path) -> Result<PathBuf, BackendError> {
    if !target.exists() {
        return Err(BackendError::TargetNotFound(target.to_path_buf()));
    }

    let c_path = match target.to_str().and_then(|s| CString::new(s).ok()) {
        Some(cp) => cp,
        None => {
            return Err(BackendError::NativeTrashError(
                "Failed to convert path to CString".into(),
            ))
        }
    };

    unsafe {
        let pool = objc_autoreleasePoolPush();

        let cls_file_manager = objc_getClass(c"NSFileManager".as_ptr());
        if cls_file_manager.is_null() {
            objc_autoreleasePoolPop(pool);
            return Err(BackendError::NativeTrashError(
                "Failed to obtain NSFileManager class".into(),
            ));
        }

        let msg_no_arg: MsgSendNoArg = std::mem::transmute(objc_msgSend as *const ());
        let msg_one_arg: MsgSendOneArg = std::mem::transmute(objc_msgSend as *const ());
        let msg_id_arg: MsgSendIdArg = std::mem::transmute(objc_msgSend as *const ());
        let msg_trash: MsgSendTrash = std::mem::transmute(objc_msgSend as *const ());
        let msg_cstr: MsgSendCStr = std::mem::transmute(objc_msgSend as *const ());

        let sel_default_mgr = sel_registerName(c"defaultManager".as_ptr());
        let mgr: *mut c_void = msg_no_arg(cls_file_manager, sel_default_mgr);
        if mgr.is_null() {
            objc_autoreleasePoolPop(pool);
            return Err(BackendError::NativeTrashError(
                "Failed to obtain NSFileManager defaultManager".into(),
            ));
        }

        let cls_nsstring = objc_getClass(c"NSString".as_ptr());
        let sel_str_with_utf8 = sel_registerName(c"stringWithUTF8String:".as_ptr());
        let ns_path: *mut c_void = msg_one_arg(cls_nsstring, sel_str_with_utf8, c_path.as_ptr());
        if ns_path.is_null() {
            objc_autoreleasePoolPop(pool);
            return Err(BackendError::NativeTrashError(
                "Failed to allocate NSString from path".into(),
            ));
        }

        let cls_nsurl = objc_getClass(c"NSURL".as_ptr());
        let sel_file_url_with_path = sel_registerName(c"fileURLWithPath:".as_ptr());
        let file_url: *mut c_void = msg_id_arg(cls_nsurl, sel_file_url_with_path, ns_path);
        if file_url.is_null() {
            objc_autoreleasePoolPop(pool);
            return Err(BackendError::NativeTrashError(
                "Failed to create NSURL for target".into(),
            ));
        }

        let sel_trash = sel_registerName(c"trashItemAtURL:resultingItemURL:error:".as_ptr());
        let mut resulting_url: *mut c_void = std::ptr::null_mut();
        let mut error: *mut c_void = std::ptr::null_mut();

        let success = msg_trash(mgr, sel_trash, file_url, &mut resulting_url, &mut error);

        if success != 0 {
            let mut dest_path = None;
            if !resulting_url.is_null() {
                let sel_path = sel_registerName(c"path".as_ptr());
                let res_ns_path: *mut c_void = msg_no_arg(resulting_url, sel_path);
                if !res_ns_path.is_null() {
                    let sel_utf8 = sel_registerName(c"UTF8String".as_ptr());
                    let c_res = msg_cstr(res_ns_path, sel_utf8);
                    if !c_res.is_null() {
                        let res_str = CStr::from_ptr(c_res).to_string_lossy().into_owned();
                        dest_path = Some(PathBuf::from(res_str));
                    }
                }
            }
            objc_autoreleasePoolPop(pool);
            Ok(dest_path.unwrap_or_else(|| PathBuf::from("~/.Trash")))
        } else {
            let mut err_msg = "Unknown native macOS trash error".to_string();
            if !error.is_null() {
                let sel_desc = sel_registerName(c"localizedDescription".as_ptr());
                let desc: *mut c_void = msg_no_arg(error, sel_desc);
                if !desc.is_null() {
                    let sel_utf8 = sel_registerName(c"UTF8String".as_ptr());
                    let c_desc = msg_cstr(desc, sel_utf8);
                    if !c_desc.is_null() {
                        err_msg = CStr::from_ptr(c_desc).to_string_lossy().into_owned();
                    }
                }
            }
            objc_autoreleasePoolPop(pool);
            Err(BackendError::NativeTrashError(err_msg))
        }
    }
}

/// Production macOS native Trash backend using Apple NSFileManager.
pub struct MacOSTrashBackend;

impl Default for MacOSTrashBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOSTrashBackend {
    pub fn new() -> Self {
        Self
    }
}

impl TrashBackend for MacOSTrashBackend {
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError> {
        #[cfg(target_os = "macos")]
        {
            native_macos_trash(target)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(BackendError::NativeTrashError(
                "Native trash is only supported on macOS".into(),
            ))
        }
    }

    fn name(&self) -> &'static str {
        "macOS Native Trash (NSFileManager.trashItemAtURL)"
    }
}

/// In-sandbox temporary trash backend for automated tests and dry environments.
pub struct TempTrashBackend {
    trash_dir: PathBuf,
}

impl TempTrashBackend {
    pub fn new(trash_dir: PathBuf) -> Self {
        Self { trash_dir }
    }
}

impl TrashBackend for TempTrashBackend {
    fn trash(&self, target: &Path) -> Result<PathBuf, BackendError> {
        if !target.exists() {
            return Err(BackendError::TargetNotFound(target.to_path_buf()));
        }

        if !self.trash_dir.exists() {
            fs::create_dir_all(&self.trash_dir)?;
        }

        let file_name = target.file_name().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid target filename")
        })?;

        let mut dest = self.trash_dir.join(file_name);
        if dest.exists() {
            let timestamp = chrono::Utc::now().timestamp_millis();
            let unique_name = format!("{}.{}", file_name.to_string_lossy(), timestamp);
            dest = self.trash_dir.join(unique_name);
        }

        match fs::rename(target, &dest) {
            Ok(_) => Ok(dest),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                if target.is_dir() {
                    copy_dir_recursive(target, &dest)?;
                    fs::remove_dir_all(target)?;
                } else {
                    fs::copy(target, &dest)?;
                    fs::remove_file(target)?;
                }
                Ok(dest)
            }
            Err(e) => Err(BackendError::Io(e)),
        }
    }

    fn name(&self) -> &'static str {
        "Temporary Test Trash"
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if entry_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_child)?;
        } else {
            fs::copy(entry.path(), &dest_child)?;
        }
    }
    Ok(())
}
