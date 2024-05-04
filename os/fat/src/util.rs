use core::ops::Range;

use crate::{sector, SectorId};

pub fn zeroize_sectors(sectors: Range<SectorId>) {
    for sid in sectors {
        sector::get(sid).lock().zeroize();
    }
}
