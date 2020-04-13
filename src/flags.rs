use crate::bitmanip::BitManipulation;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
	pub fn serialize_flag_storage(self) -> u32 {
		let mut flag_store: u32 = 0;
		flag_store.set_bit(0.into(), self.carry);
		flag_store.set_bit(1.into(), true);
		flag_store.set_bit(2.into(), self.parity);
		flag_store.set_bit(4.into(), self.adjust);
		flag_store.set_bit(6.into(), self.zero);
		flag_store.set_bit(7.into(), self.sign);
		flag_store.set_bit(8.into(), self.trap);
		flag_store.set_bit(9.into(), self.interrupt);
		flag_store.set_bit(10.into(), self.direction);
		flag_store.set_bit(11.into(), self.overflow);
		flag_store
	}
	pub fn deserialize_flag_storage(&mut self, flag_store: u32) {
		self.carry = flag_store.get_bit(0.into());
		self.parity = flag_store.get_bit(2.into());
		self.adjust = flag_store.get_bit(4.into());
		self.zero = flag_store.get_bit(6.into());
		self.sign = flag_store.get_bit(7.into());
		self.trap = flag_store.get_bit(8.into());
		self.interrupt = flag_store.get_bit(9.into());
		self.direction = flag_store.get_bit(10.into());
		self.overflow = flag_store.get_bit(11.into());
	}
}
