use x86::{
    current::paging::BASE_PAGE_SHIFT,
    dtables::DescriptorTablePointer,
    segmentation::{
        Descriptor,
        SegmentSelector,
    },
};

pub fn get_segment_base(
    table: &DescriptorTablePointer<Descriptor>,
    selector: SegmentSelector,
) -> u32 {
    let entry = unsafe { &*table.base.offset(selector.index() as isize) };
    let base_0 = entry.lower & 0xFFFF;
    let base_1 = entry.upper & 0xFF;
    let base_2 = entry.upper >> 24;
    base_0 | (base_1 << 16) | (base_2 << 24)
}

pub fn get_segment_access_right(
    table: &DescriptorTablePointer<Descriptor>,
    selector: SegmentSelector,
) -> u32 {
    const VMX_SEGMENT_ACCESS_RIGHTS_UNUSABLE_FLAG: u32 = 1 << 16;
    if selector.index() == 0 && (selector.bits() >> 2) == 0 {
        return VMX_SEGMENT_ACCESS_RIGHTS_UNUSABLE_FLAG;
    }

    let entry = unsafe { &*table.base.offset(selector.index() as isize) };

    // Get the Type, S, DPL, P, AVL, L, D/B and G bits from the segment descriptor.
    // See: Figure 3-8. Segment Descriptor
    let ar = (entry.as_u64() >> 40) as u32;
    ar & 0b1111_0000_1111_1111
}

/// Returns the limit of the given segment.
pub fn get_segment_limit(
    table: &DescriptorTablePointer<Descriptor>,
    selector: SegmentSelector,
) -> u32 {
    if selector.index() == 0 && (selector.bits() >> 2) == 0 {
        return 0; // unusable
    }

    let entry = unsafe { &*table.base.offset(selector.index() as isize) };
    let limit_low = entry.as_u64() & 0xffff;
    let limit_high = (entry.as_u64() >> (32 + 16)) & 0xF;
    let mut limit = limit_low | (limit_high << 16);
    if ((entry.as_u64() >> (32 + 23)) & 0x01) != 0 {
        limit = ((limit + 1) << BASE_PAGE_SHIFT) - 1;
    }
    limit as u32
}
