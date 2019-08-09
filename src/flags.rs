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
	pub fn calculate_zero(&mut self, result: u32) {
		self.zero = result == 0;
	}
}
