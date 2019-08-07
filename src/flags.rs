#[derive(Debug, Default, PartialEq)]
pub struct X86Flags {
	pub carry: bool,
	pub parity: bool,
	pub adjust: bool,
	pub zero: bool,
	pub sign: bool,
	pub trap: bool,
	pub interrupt: bool,
	pub direction: bool,
	pub overflow: bool
}

impl X86Flags {
	pub fn calculate_parity(&mut self, val: u32) {
		let mut count = 0;
		for i in 0..8 {
			if (val&((1<<i)))!=0{
				count+=1;
			}
		}
		self.parity = (count % 2) == 0;
	}
	pub fn calculate_sign8(&mut self, val: u8) {
		if (val & 0x80) != 0 {
			self.sign = true;
		} else {
			self.sign = false;
		}
	}
	pub fn calculate_sign16(&mut self, val: u16) {
		if (val & 0x8000) != 0 {
			self.sign = true;
		} else {
			self.sign = false;
		}
	}
	pub fn calculate_sign32(&mut self, val: u32) {
		if (val & 0x80000000) != 0 {
			self.sign = true;
		} else {
			self.sign = false;
		}
	}
	pub fn calculate_overflow8(&mut self, result: u8, val1: u8, val2: u8) {
		self.overflow = !((val1^val2) & 0x80 != 0) && ((val1^result) & 0x80 != 0);
	}
	pub fn calculate_overflow16(&mut self, result: u16, val1: u16, val2: u16) {
		self.overflow = !((val1^val2) & 0x80 != 0) && ((val1^result) & 0x8000 != 0);
	}
	pub fn calculate_overflow32(&mut self, result: u32, val1: u32, val2: u32) {
		self.overflow =  !((val1^val2) & 0x80 != 0) && ((val1^result) & 0x80000000 != 0);
	}
	pub fn calculate_zero(&mut self, result: u32) {
		self.zero = result == 0;
	}
}
