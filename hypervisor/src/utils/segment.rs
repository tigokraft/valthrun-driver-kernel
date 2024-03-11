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
) -> u64 {
    let entry = unsafe { &*table.base.offset(selector.index() as isize) };
    let value = entry.as_u64();

    // LA_ACCESSED
    let attr_la = (entry.as_u64() >> 44) & 0x01;

    let base_0 = (value >> 16) & 0xFFFF;
    let base_1 = (value >> 32) & 0xFF;
    let base_2 = (value >> 56) & 0xFF;
    let base = base_0 | (base_1 << 16) | (base_2 << 24);

    if attr_la > 0 {
        base
    } else {
        /* this is a TSS or callgate etc, save the base high part */
        /* TODO: Properly handle this maybe... */
        base
    }
}

pub fn get_segment_access_right(
    table: &DescriptorTablePointer<Descriptor>,
    selector: SegmentSelector,
) -> u64 {
    const VMX_SEGMENT_ACCESS_RIGHTS_UNUSABLE_FLAG: u64 = 1 << 16;
    if selector.index() == 0 && (selector.bits() >> 2) == 0 {
        return VMX_SEGMENT_ACCESS_RIGHTS_UNUSABLE_FLAG;
    }

    let entry = unsafe { &*table.base.offset(selector.index() as isize) };

    // Get the Type, S, DPL, P, AVL, L, D/B and G bits from the segment descriptor.
    // See: Figure 3-8. Segment Descriptor
    (entry.as_u64() >> 40) & 0b1111_0000_1111_1111

    // let attributes_0 = (entry.as_u64() >> 40) & 0xFF;
    // let attributes_1 = (entry.as_u64() >> 52) & 0x0F;
    // let attributes = attributes_0 | (attributes_1 << 0x08);
    // attributes
}

/// Returns the limit of the given segment.
pub fn get_segment_limit(
    table: &DescriptorTablePointer<Descriptor>,
    selector: SegmentSelector,
) -> u64 {
    if selector.index() == 0 && (selector.bits() >> 2) == 0 {
        return 0; // unusable
    }

    let entry = unsafe { &*table.base.offset(selector.index() as isize) };
    let value = entry.as_u64();

    let limit_0 = (value >> 0) & 0xFFFF;
    let limit_1 = (value >> 48) & 0x0F;

    let mut limit = limit_0 | (limit_1 << 16);
    let flag_g = (entry.as_u64() >> 55) & 0x01;
    if flag_g > 0 {
        /* 4096-bit granularity is enabled for this segment, scale the limit */
        limit = ((limit + 1) << BASE_PAGE_SHIFT) - 1;
    }
    limit
}
