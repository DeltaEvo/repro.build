use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter;
use std::rc::Rc;

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub struct Inode(pub u64);

impl Inode {
    fn from_index(index: usize) -> Self {
        Self(index as u64 + 2)
    }

    pub fn is_root(&self) -> bool {
        self.0 == 1
    }

    fn as_index(&self) -> usize {
        let ino = self.0;
        assert_ne!(ino, 0, "null inode");
        assert_ne!(ino, 1, "root inode");
        ino as usize - 2
    }
}

struct Entry {
    parent: Inode,
    name: Rc<OsStr>,
    nlookup: usize,
    generation: u64,
}

#[derive(Hash, Eq, PartialEq)]
struct Key {
    parent: Inode,
    name: Rc<OsStr>,
}

#[derive(Default)]
pub struct Table {
    entries: Vec<Entry>,
    by_name: HashMap<Key, Inode>,
}

impl Table {
    pub fn name(&self, inode: Inode) -> Rc<OsStr> {
        self.entries[inode.as_index()].name.clone()
    }

    pub fn parent(&self, inode: Inode) -> Inode {
        self.entries[inode.as_index()].parent
    }

    pub fn generation(&self, inode: Inode) -> u64 {
        self.entries[inode.as_index()].generation
    }

    pub fn parents(&self, inode: Inode) -> impl Iterator<Item = Rc<OsStr>> + '_ {
        let mut parent = self.entries[inode.as_index()].parent;
        iter::from_fn(move || {
            if parent.is_root() {
                None
            } else {
                let entry = &self.entries[parent.as_index()];
                parent = entry.parent;
                Some(entry.name.clone())
            }
        })
    }

    pub fn lookup(&mut self, parent: Inode, name: &OsStr) -> Inode {
        // FIXME: we are allocating for each lookup
        let name: Rc<OsStr> = name.into();

        let ino = *self
            .by_name
            .entry(Key { parent, name })
            .or_insert_with_key(|key| {
                self.entries.push(Entry {
                    parent: key.parent,
                    name: key.name.clone(),
                    nlookup: 0,
                    generation: 0, // TODO: time
                });

                Inode::from_index(self.entries.len() - 1)
            });

        let entry = &mut self.entries[ino.as_index()];

        entry.nlookup += 1;

        ino
    }
}
