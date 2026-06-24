use std::ffi::c_void;

extern "system" {
    fn VirtualProtect(addr: *mut c_void, size: usize, new_prot: u32, old_prot: *mut u32) -> i32;
}

// PAGE_EXECUTE_READWRITE
const PAGE_EXEC_RW: u32 = 0x40;

// writes a 14-byte absolute jmp (mov rax, addr; jmp rax) at `at`
unsafe fn write_abs_jmp(at: *mut u8, to: usize) {
    at.write(0x48);              // REX.W
    at.add(1).write(0xB8);       // mov rax,
    (at.add(2) as *mut u64).write(to as u64);
    at.add(10).write(0xFF);      // jmp rax
    at.add(11).write(0xE0);
}

pub struct Hook {
    pub trampoline: Vec<u8>,
    target: *mut u8,
}

unsafe impl Send for Hook {}
unsafe impl Sync for Hook {}

impl Hook {
    // patches `target` to jump to `dest`, returns a trampoline you call to run the original
    pub unsafe fn install(target: *mut u8, dest: usize) -> Self {
        const PATCH_LEN: usize = 14;

        let mut old_prot: u32 = 0;
        VirtualProtect(target as *mut c_void, PATCH_LEN, PAGE_EXEC_RW, &mut old_prot);

        // copy original bytes + a jmp back into the trampoline
        let mut trampoline = vec![0u8; PATCH_LEN + 14];
        std::ptr::copy_nonoverlapping(target, trampoline.as_mut_ptr(), PATCH_LEN);
        write_abs_jmp(trampoline.as_mut_ptr().add(PATCH_LEN), target.add(PATCH_LEN) as usize);

        // make trampoline executable
        let mut tmp: u32 = 0;
        VirtualProtect(trampoline.as_mut_ptr() as *mut c_void, trampoline.len(), PAGE_EXEC_RW, &mut tmp);

        // patch the target
        write_abs_jmp(target, dest);

        VirtualProtect(target as *mut c_void, PATCH_LEN, old_prot, &mut old_prot);

        Hook { trampoline, target }
    }

    // call this to invoke the original function before our hook runs
    pub unsafe fn call_original(&self) -> unsafe extern "C" fn() {
        std::mem::transmute(self.trampoline.as_ptr())
    }
}
