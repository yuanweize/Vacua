use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedEntry {
    pub path: PathBuf,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub inode: u64,
    pub device_id: u64,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_sparse: bool,
    pub is_clone: bool,
    pub clone_id: Option<u64>,
    pub clone_refcnt: u32,
    pub nlink: u64,
    pub mtime_sec: i64,
    pub ctime_sec: i64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Default)]
struct AttrList {
    bitmapcount: u16,
    reserved: u16,
    commonattr: u32,
    volattr: u32,
    dirattr: u32,
    fileattr: u32,
    forkattr: u32,
}

#[cfg(target_os = "macos")]
#[repr(C, packed(4))]
#[derive(Default, Debug)]
struct AttrBufClone {
    length: u32,
    clone_id: u64,
    ext_flags: u64,
    clone_refcnt: u32,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn getattrlist(
        path: *const std::ffi::c_char,
        attrList: *mut std::ffi::c_void,
        attrBuf: *mut std::ffi::c_void,
        attrBufSize: usize,
        options: u32,
    ) -> i32;
}

#[cfg(target_os = "macos")]
fn query_apfs_clone_attributes(path: &Path) -> (Option<u64>, u32, bool, bool) {
    use std::ffi::CString;

    let c_path = match path.to_str().and_then(|s| CString::new(s).ok()) {
        Some(cp) => cp,
        None => return (None, 1, false, false),
    };

    let mut al = AttrList::default();
    al.bitmapcount = 5;
    // ATTR_CMNEXT_CLONEID (0x100) | ATTR_CMNEXT_EXT_FLAGS (0x200) | ATTR_CMNEXT_CLONE_REFCNT (0x1000)
    al.forkattr = 0x00000100 | 0x00000200 | 0x00001000;

    let mut buf = AttrBufClone::default();

    let res = unsafe {
        getattrlist(
            c_path.as_ptr(),
            &mut al as *mut _ as *mut std::ffi::c_void,
            &mut buf as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<AttrBufClone>(),
            0x00000020, // FSOPT_ATTR_CMN_EXTENDED
        )
    };

    if res == 0 {
        // EF_MAY_SHARE_BLOCKS = 0x01, EF_SHARES_ALL_BLOCKS = 0x40
        let is_cloned = buf.clone_refcnt > 1 || (buf.ext_flags & 0x41) != 0;
        let is_sparse = (buf.ext_flags & 0x10) != 0;
        (
            Some(buf.clone_id),
            buf.clone_refcnt.max(1),
            is_cloned,
            is_sparse,
        )
    } else {
        (None, 1, false, false)
    }
}

impl ScannedEntry {
    #[cfg(unix)]
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(&path)?;
        let is_symlink = meta.file_type().is_symlink();
        let is_dir = meta.is_dir();

        let logical_bytes = meta.len();
        // On macOS / APFS / HFS+, blocks are 512-byte units
        let allocated_bytes = meta.blocks() * 512;
        let mut is_sparse = !is_dir && !is_symlink && (allocated_bytes < logical_bytes);
        let mut is_clone = false;
        let mut clone_id = None;
        let mut clone_refcnt = 1;

        #[cfg(target_os = "macos")]
        if !is_dir && !is_symlink && logical_bytes > 0 {
            let (cid, refcnt, cloned, sparse_flag) = query_apfs_clone_attributes(&path);
            clone_id = cid;
            clone_refcnt = refcnt;
            is_clone = cloned;
            is_sparse = is_sparse || sparse_flag;
        }

        Ok(Self {
            path,
            logical_bytes,
            allocated_bytes,
            inode: meta.ino(),
            device_id: meta.dev(),
            is_dir,
            is_symlink,
            is_sparse,
            is_clone,
            clone_id,
            clone_refcnt,
            nlink: meta.nlink(),
            mtime_sec: meta.mtime(),
            ctime_sec: meta.ctime(),
        })
    }
}
