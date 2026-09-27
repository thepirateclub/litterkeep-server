//! Фрагмент non-L Bidi_Class ranges Unicode 17.
use super::super::BidiClass;

pub(super) const RANGES: &[(u32, u32, BidiClass)] = &[
    (0x06FFFE, 0x06FFFF, BidiClass::BoundaryNeutral),
    (0x07FFFE, 0x07FFFF, BidiClass::BoundaryNeutral),
    (0x08FFFE, 0x08FFFF, BidiClass::BoundaryNeutral),
    (0x09FFFE, 0x09FFFF, BidiClass::BoundaryNeutral),
    (0x0AFFFE, 0x0AFFFF, BidiClass::BoundaryNeutral),
    (0x0BFFFE, 0x0BFFFF, BidiClass::BoundaryNeutral),
    (0x0CFFFE, 0x0CFFFF, BidiClass::BoundaryNeutral),
    (0x0DFFFE, 0x0E00FF, BidiClass::BoundaryNeutral),
    (0x0E0100, 0x0E01EF, BidiClass::NonspacingMark),
    (0x0E01F0, 0x0E0FFF, BidiClass::BoundaryNeutral),
    (0x0EFFFE, 0x0EFFFF, BidiClass::BoundaryNeutral),
    (0x0FFFFE, 0x0FFFFF, BidiClass::BoundaryNeutral),
    (0x10FFFE, 0x10FFFF, BidiClass::BoundaryNeutral),
];
