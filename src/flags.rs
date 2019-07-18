pub struct X86Flags {
    pub carry_flag: bool,
    pub parity_flag: bool,
    pub adjust_flag: bool,
    pub zero_flag: bool,
    pub sign_flag: bool,
    pub trap_flag: bool,
    pub interrupt_flag: bool,
    pub direction_flag: bool,
    pub overflow_flag: bool,
    pub iopl_flag: bool,
    pub nested_task_flag: bool,
    pub resume_flag: bool,
    pub virtual_8086_mode_flag: bool,
    pub alignment_check_flag: bool,
    pub id_capability_flag: bool,
    pub vif_flag: bool,
    pub vip_flag: bool
}

impl X86Flags {
    pub fn calculate_parity_flag(&mut self, u32 val) {
        let mut count = 0;
        for i in 0..7 {
            if ((val&((1<<i)))!=0){
                count++;
            }
        }
        self.parity_flag = (count % 2) == 0;
    }
    pub fn calculate_sign_flag8(&mut self, u8 val) {
        if (val & 0x80) == 0 {
            self.sign_flag = true;
        } else {
            self.sign_flag = false;
        }
    }
    pub fn calculate_sign_flag16(&mut self, u16 val) {
        if (val & 0x8000) == 0 {
            self.sign_flag = true;
        } else {
            self.sign_flag = false;
        }
    }
    pub fn calculate_sign_flag32(&mut self, u32 val) {
        if (val & 0x80000000) == 0 {
            self.sign_flag = true;
        } else {
            self.sign_flag = false;
        }
    }
    pub fn calculate_overflow_flag8(&mut self, u8 result, u8 val1, u8 val2) {
        self.overflow_flag = !((v1^v2) & 0x80) && ((v1^result) & 0x80);
    }
    pub fn calculate_overflow_flag16(&mut self, u16 result, u16 val1, u16 val2) {
        self.overflow_flag = !((v1^v2) & 0x80) && ((v1^result) & 0x8000);
    }
    pub fn calculate_overflow_flag32(&mut self, u32 result, u32 val1, u32 val2) {
        self.overflow_flag =  !((v1^v2) & 0x80) && ((v1^result) & 0x80000000);
    }
    pub fn calculate_zero_flag(&mut self, u32 result) {
        self.zero_flag = result == 0;
    }
}
