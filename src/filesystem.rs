use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use fuser::{ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request};
use libc::{ENOENT, ENOSYS};

mod control;
mod inode;

const NAME: &'static str = env!("CARGO_PKG_NAME");
const TTL: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Filesystem {
    inodes: inode::Table,
}

impl Filesystem {
    fn new() -> Self {
        Default::default()
    }
}

impl fuser::Filesystem for Filesystem {
    fn lookup(&mut self, req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let ino = self.inodes.lookup(inode::Inode(parent), name);

        if control::is_inside(&self.inodes, ino) {
            reply.entry(
                &TTL,
                &fuser::FileAttr {
                    ino: ino.0,
                    size: 0,
                    blocks: 0,
                    atime: UNIX_EPOCH,
                    mtime: UNIX_EPOCH,
                    ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH,
                    kind: control::file_type(&self.inodes, ino).unwrap(),
                    perm: 0o755,
                    nlink: 1,
                    uid: req.uid(),
                    gid: req.gid(),
                    rdev: 0,
                    flags: 0,
                    blksize: 0,
                },
                self.inodes.generation(ino),
            );
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let ino = inode::Inode(ino);
        let parent_ino = if ino.is_root() {
            inode::Inode(0)
        } else {
            self.inodes.parent(ino)
        };

        let reply = move |entries: Result<
            &mut dyn Iterator<Item = (Cow<'static, OsStr>, fuser::FileType)>,
            i32,
        >| {
            let entries = match entries {
                Ok(entries) => entries,
                Err(err) => return reply.error(err),
            };
            let entries = entries.map(|(name, file_type)| (name, 0, file_type));

            let extra = [
                (".", ino.0, fuser::FileType::Directory),
                ("..", parent_ino.0, fuser::FileType::Directory),
            ]
            .map(|(name, ino, file_type)| (OsStr::new(name).into(), ino, file_type))
            .into_iter();

            for (index, entry) in extra.chain(entries).enumerate().skip(offset as usize) {
                let (name, inode, file_type) = entry;

                // i + 1 means the index of the next entry
                let end = reply.add(inode, index as i64 + 1, file_type, name);

                if end {
                    break;
                }
            }
            reply.ok();
        };

        if ino.is_root() {
            let mut entries = [
                (control::ROOT_DIRECTORY, fuser::FileType::Directory),
                // ("hello", fuser::FileType::RegularFile),
            ]
            .into_iter()
            .map(|(name, file_type)| (OsStr::new(name).into(), file_type));
            reply(Ok(&mut entries));
        } else if control::is_inside(&self.inodes, ino) {
            let mut entries = control::readdir(&self.inodes, ino)
                .expect("file is not a directory")
                .iter()
                .map(|file| {
                    (
                        OsStr::new(*file).into(),
                        control::file_type(&self.inodes, ino).unwrap(),
                    )
                });
            reply(Ok(&mut entries));
        } else {
            return reply(Err(ENOENT));
        };
    }

    fn open(&mut self, _req: &Request<'_>, _ino: u64, _flags: i32, reply: ReplyOpen) {
        // TODO: only use direct io for virtual files
        reply.opened(0, fuser::consts::FOPEN_DIRECT_IO);
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let ino = inode::Inode(ino);

        if control::is_inside(&self.inodes, ino) {
            let data = control::read(&self.inodes, ino).unwrap();

            let start = offset as usize;
            let end = offset as usize + size as usize;

            if start > data.len() {
                reply.data(&[]);
            } else if end > data.len() {
                reply.data(&data[start..]);
            } else {
                reply.data(&data[start..end]);
            }
        } else {
            reply.error(ENOSYS);
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let parent = inode::Inode(parent);

        if control::is_inside(&self.inodes, parent) {
            control::unlink(&self.inodes, parent, name, reply);
        } else {
            reply.error(ENOSYS);
        }
    }

    fn getattr(&mut self, req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        let ino = inode::Inode(ino);

        let kind = if ino.is_root() {
            fuser::FileType::Directory
        } else {
            control::file_type(&self.inodes, ino).unwrap()
        };

        reply.attr(
            &TTL,
            &fuser::FileAttr {
                ino: ino.0,
                size: 0,
                blocks: 0,

                atime: UNIX_EPOCH,
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind,
                perm: 0o755,
                nlink: 1,
                uid: req.uid(),
                gid: req.gid(),
                rdev: 0,
                flags: 0,
                blksize: 0,
            },
        );
    }
}

pub fn mount(mountpoint: &Path) -> io::Result<()> {
    fuser::mount2(
        Filesystem::new(),
        mountpoint,
        &[
            fuser::MountOption::RW,
            fuser::MountOption::FSName(NAME.to_string()),
            fuser::MountOption::AllowOther,
            fuser::MountOption::AutoUnmount,
        ],
    )
}

#[derive(Debug)]
pub struct Remote {
    dir: PathBuf,
}

pub fn remote(mountpoint: &Path) -> Option<Remote> {
    let control_directory = mountpoint.join(control::ROOT_DIRECTORY);

    if control_directory.exists() {
        Some(Remote {
            dir: control_directory,
        })
    } else {
        None
    }
}

impl Remote {
    pub fn pid(&self) -> io::Result<u32> {
        let pid = fs::read(self.dir.join("pid"))?;
        let pid = String::from_utf8(pid).expect("pid is not utf8");
        let pid = pid.parse().expect("pid is not a number");
        Ok(pid)
    }

    pub fn unmount(self) -> io::Result<()> {
        fs::remove_file(self.dir.join("pid"))
    }
}
