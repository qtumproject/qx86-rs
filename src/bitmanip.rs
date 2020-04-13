
pub trait BitManipulation{
    fn get_bit(&self, index: u8) -> bool;
    fn set_bit(&mut self, index: u8, value: bool); 
    fn get_bit_big_endian(&self, index: u8) -> bool;
}

impl BitManipulation for u32{
    fn get_bit(&self, index: u8) -> bool{
        self & (1 << index) > 0
    }
    fn get_bit_big_endian(&self, index: u8) -> bool{
        self & (1 << (31 - index)) > 0
    }
    fn set_bit(&mut self, index: u8, value: bool){
        if value{
            *self = *self | (1 << index);
        }else{
            *self = *self & (0xFFFFFFFF ^ (1 << index));
        }
    }
}

impl BitManipulation for u16{
    fn get_bit(&self, index: u8) -> bool{
        *self & (1 << index) > 0
    }
    fn get_bit_big_endian(&self, index: u8) -> bool{
        self & (1 << (15 - index)) > 0
    }
    fn set_bit(&mut self, index: u8, value: bool){
        if value{
            *self = *self | (1 << index);
        }else{
            *self = *self & (0xFFFF ^ (1 << index));
        }
    }
}

impl BitManipulation for u8{
    fn get_bit(&self, index: u8) -> bool{
        self & (1 << index) > 0
    }
    fn get_bit_big_endian(&self, index: u8) -> bool{
        self & (1 << (7 - index)) > 0
    }
    fn set_bit(&mut self, index: u8, value: bool){
        if value{
            *self = *self | (1 << index);
        }else{
            *self = *self & (0xFF ^ (1 << index));
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn test_bits(){
        let tmp = 0b0000_1000u8;
        assert!(!tmp.get_bit(0));
        assert!(tmp.get_bit(3));
        assert!(tmp.get_bit_big_endian(4));
        let mut tmp2 = tmp;
        tmp2.set_bit(2, true);
        tmp2.set_bit(3, false);
        tmp2.set_bit(0, true);
        assert!(tmp2 == 0b0000_0101);
        assert!(tmp2.get_bit(0));
        assert!(tmp2.get_bit_big_endian(7));
    }
}
