//! PlayerFurnacesList: the furnaces each player placed, kept on the player (an attachment, saved with them, as the
//! Java mod's `PLAYER_FURNACES_LIST` attachment). The Rainbow Furnace looks through its owner's.

use crate::util::*;
use mod_api::*;

fn encode(list: &Vec<Pos>) -> Result<Compound, AttachmentError> {
    let mut flat = Vec::with_capacity(list.len() * 3);
    for p in list {
        flat.extend_from_slice(p);
    }
    let mut c = Compound::new();
    c.insert(java("furnaces"), Tag::IntArray(flat));
    Ok(c)
}

fn decode(c: &Compound) -> Result<Vec<Pos>, AttachmentError> {
    let flat = int_array(c, "furnaces", 0);
    Ok(flat.as_chunks::<3>().0.iter().map(|p| [p[0], p[1], p[2]]).collect())
}

pub(crate) static FURNACES_LIST: AttachmentType<Vec<Pos>> = AttachmentType {
    namespace: "ironfurnaces",
    name: "player_furnaces_list",
    default: Vec::new,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: true,
    sync: false,
};

pub(crate) fn add(host: &mut dyn ServerHost, player: u128, pos: Pos) {
    let holder = Holder::Player(player);
    let mut list = FURNACES_LIST.get(host.attachments(), holder).unwrap_or_default();
    if !list.contains(&pos) {
        list.push(pos);
        let _ = FURNACES_LIST.set(host.attachments(), holder, list);
    }
}

pub(crate) fn list(host: &mut dyn ServerHost, player: u128) -> Vec<Pos> {
    FURNACES_LIST.get_existing(host.attachments(), Holder::Player(player)).ok().flatten().unwrap_or_default()
}
