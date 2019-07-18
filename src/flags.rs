pub struct X86Flags {
    pub carry: bool,
    pub parity: bool,
    pub adjust: bool,
    pub zero: bool,
    pub sign: bool,
    pub trap: bool,
    pub interrupt: bool,
    pub direction: bool,
    pub overflow: bool,
    pub iopl: bool,
    pub nested_task: bool,
    pub resume: bool,
    pub virtual_8086_mode: bool,
    pub alignment_check: bool,
    pub id_capability: bool,
    pub vif: bool,
    pub vip: bool
}

impl X86Flags {
    pub fn calculate_parity(&mut self, u32 val) {
        let mut count = 0;
        for i in 0..7 {
            if ((val&((1<<i)))!=0){
                count++;
            }
        }
        self.parity = (count % 2) == 0;
    }
    pub fn calculate_sign8(&mut self, u8 val) {
        if (val & 0x80) == 0 {
            self.sign = true;
        } else {
            self.sign = false;
        }
    }
    pub fn calculate_sign16(&mut self, u16 val) {
        if (val & 0x8000) == 0 {
            self.sign = true;
        } else {
            self.sign = false;
        }
    }
    pub fn calculate_sign32(&mut self, u32 val) {
        if (val & 0x80000000) == 0 {
            self.sign = true;
        } else {
            self.sign = false;
        }
    }
    pub fn calculate_overflow8(&mut self, u8 result, u8 val1, u8 val2) {
        self.overflow = !((v1^v2) & 0x80) && ((v1^result) & 0x80);
    }
    pub fn calculate_overflow16(&mut self, u16 result, u16 val1, u16 val2) {
        self.overflow = !((v1^v2) & 0x80) && ((v1^result) & 0x8000);
    }
    pub fn calculate_overflow32(&mut self, u32 result, u32 val1, u32 val2) {
        self.overflow =  !((v1^v2) & 0x80) && ((v1^result) & 0x80000000);
    }
    pub fn calculate_zero(&mut self, u32 result) {
        self.zero = result == 0;
    }
}
