use spin::Once;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();
static TSS: Once<TaskStateSegment> = Once::new();

/// Initialize the GDT, TSS and CS.
///
/// # Safety
/// Must only be called once.
pub unsafe fn init() {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        let stack_start = VirtAddr::from_ptr(&STACK);
        stack_start + STACK_SIZE
    };

    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
    let tss_selector = gdt.add_entry(Descriptor::tss_segment(TSS.call_once(|| tss)));
    let gdt = GDT.call_once(|| {
        (
            gdt,
            Selectors {
                code_selector,
                tss_selector,
            },
        )
    });

    gdt.0.load();
    x86_64::instructions::segmentation::set_cs(gdt.1.code_selector);
    x86_64::instructions::tables::load_tss(gdt.1.tss_selector);
}
