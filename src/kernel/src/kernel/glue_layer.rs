#![no_std]
use crate::sync::{Mutex, Lazy};
use hashbrown::HashMap;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum SysErr {
    Ok = 0,
    EACCES = 1,
    ENOENT = 2,
    EINVAL = 22,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct GlueStatus {
    pub inst_id: u64,
    pub state: u32,
    pub stat_a: u64,
    pub stat_b: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum GlueCommand {
    DoWork(u32),
    Close,
}

static LIB_INST_TABLE: Lazy<Mutex<HashMap<u64, ()>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_INST_ID: Mutex<u64> = Mutex::new(0);

fn lib_inst_table() -> &'static Mutex<HashMap<u64, ()>> {
    Lazy::force(&LIB_INST_TABLE)
}

pub fn init() {
    crate::println!("glue translation layer init done");
}

pub fn alloc_inst_id() -> u64 {
    let mut id = NEXT_INST_ID.lock();
    *id += 1;
    let new_id = *id;
    lib_inst_table().lock().insert(new_id, ());
    new_id
}

pub fn glue_entry_read(lib_inst_id: u64) -> Result<Vec<u8>, SysErr> {
    if !lib_inst_table().lock().contains_key(&lib_inst_id) {
        return Err(SysErr::ENOENT);
    }
    let our_status = GlueStatus {
        inst_id: lib_inst_id,
        state: 1,
        stat_a: 100,
        stat_b: 200,
    };
    serde_json::to_vec(&our_status).map_err(|_| SysErr::EINVAL)
}

pub fn glue_entry_write(lib_inst_id: u64, input: &[u8]) -> Result<(), SysErr> {
    let cmd: GlueCommand = serde_json::from_slice(input).map_err(|_| SysErr::EINVAL)?;
    let mut table = lib_inst_table().lock();
    if !table.contains_key(&lib_inst_id) {
        return Err(SysErr::ENOENT);
    }
    match cmd {
        GlueCommand::DoWork(_arg) => {}
        GlueCommand::Close => {
            table.remove(&lib_inst_id);
        }
    }
    Ok(())
}

pub fn glue_on_kobject_delete(lib_inst_id: u64) {
    lib_inst_table().lock().remove(&lib_inst_id);
}
