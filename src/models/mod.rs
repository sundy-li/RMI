// < begin copyright > 
// Copyright Ryan Marcus 2020
// 
// See root directory of this project for license terms.
// 
// < end copyright > 
 
 

mod balanced_radix;
mod bottom_up_plr;
mod cubic_spline;
mod histogram;
mod linear;
mod linear_spline;
mod normal;
mod pgm;
mod radix;
mod stdlib;
mod utils;

pub use balanced_radix::BalancedRadixModel;
pub use bottom_up_plr::BottomUpPLR;
pub use cubic_spline::CubicSplineModel;
pub use histogram::EquidepthHistogramModel;
pub use linear::LinearModel;
pub use linear::RobustLinearModel;
pub use linear::LogLinearModel;
pub use linear_spline::LinearSplineModel;
pub use normal::LogNormalModel;
pub use normal::NormalModel;
pub use pgm::PGM;
pub use radix::RadixModel;
pub use radix::RadixTable;
pub use stdlib::StdFunctions;

use std::collections::HashSet;
use std::io::Write;
use byteorder::{WriteBytesExt, LittleEndian};
use superslice::*;

#[derive(Clone)]
pub struct ModelDataWrapper<'a> {
    model_data: &'a ModelData,
    scaling_factor: f64
}

impl <'a> ModelDataWrapper<'a> {
    pub fn new(md: &'a ModelData) -> ModelDataWrapper<'a> {
        return ModelDataWrapper {
            model_data: md,
            scaling_factor: 1.0
        }
    }

    pub fn set_scale(&mut self, scale: f64) {
        self.scaling_factor = scale;
    }

    pub fn len(&self) -> usize {
        return self.model_data.len();
    }

    pub fn get(&self, idx: usize) -> (f64, f64) {
        let (x, y) = self.model_data.get(idx);
        return (x, y * self.scaling_factor);
    }

    pub fn get_key(&self, idx: usize) -> u64 {
        return self.model_data.get_key(idx);
    }
    
    #[allow(dead_code)]
    pub fn lower_bound(&self, lookup: u64) -> usize {
        return self.as_int_int().lower_bound_by(|(k, _)| k.cmp(&lookup));
    }

    pub fn iter_float_float(&self) -> ModelDataFFIterator {
        let mut iter = self.model_data.iter_float_float();
        iter.set_scale(self.scaling_factor);
        return iter;
    }
    
    pub fn iter_int_int(&self) -> ModelDataIIIterator {
        let mut iter = self.model_data.iter_int_int();
        iter.set_scale(self.scaling_factor);
        return iter;
    }

    pub fn as_int_int(&self) -> &[(u64, u64)] {
        return self.model_data.as_int_int();
    }

    pub fn into_data(self) -> ModelData {
        return self.model_data.clone();
    }
}

#[derive(Clone)]
pub enum ModelData {
    IntKeyToIntPos(Vec<(u64, u64)>),
    #[allow(dead_code)]
    FloatKeyToIntPos(Vec<(f64, u64)>),
    #[allow(dead_code)]
    IntKeyToFloatPos(Vec<(u64, f64)>),
    FloatKeyToFloatPos(Vec<(f64, f64)>),
}

#[cfg(test)]
macro_rules! vec_to_ii {
    ($x:expr) => {
        ($x).into_iter()
            .map(|(x, y)| (x as u64, y as u64))
            .collect()
    };
}

macro_rules! extract_and_convert_tuple {
    ($vec: expr, $idx: expr, $type1:ty, $type2:ty, $scale: expr) => {{
        let (x, y) = $vec[$idx];
        (x as $type1, (y as f64 * $scale) as $type2)
    }};
}


macro_rules! define_iterator_type {
    ($name: tt, $type1: ty, $type2: ty) => {
        pub struct $name<'a> {
            data: &'a ModelData,
            idx: usize,
            scale: f64,
            stop: usize
        }

        impl<'a> $name<'a> {
            fn new(data: &'a ModelData) -> $name<'a> {
                return $name { data: data, idx: 0, scale: 1.0, stop: data.len() };
            }

            fn set_scale(&mut self, scale: f64) {
                self.scale = scale;
            }

            pub fn bound(&mut self, start: usize, stop: usize) {
                assert!(start < stop);
                assert!(stop <= self.data.len());
                self.idx = start;
                self.stop = stop;
            }
        }

        impl<'a> Iterator for $name<'a> {
            type Item = ($type1, $type2);

            fn next(&mut self) -> Option<Self::Item> {
                if self.idx >= self.stop {
                    return None;
                }

                let itm = match self.data {
                    ModelData::FloatKeyToFloatPos(data) => {
                        extract_and_convert_tuple!(data, self.idx, $type1, $type2, self.scale)
                    }
                    ModelData::FloatKeyToIntPos(data) => {
                        extract_and_convert_tuple!(data, self.idx, $type1, $type2, self.scale)
                    }
                    ModelData::IntKeyToIntPos(data) => {
                        extract_and_convert_tuple!(data, self.idx, $type1, $type2, self.scale)
                    }
                    ModelData::IntKeyToFloatPos(data) => {
                        extract_and_convert_tuple!(data, self.idx, $type1, $type2, self.scale)
                    }
                };
                self.idx += 1;

                return Some(itm);
            }
        }
    };
}


define_iterator_type!(ModelDataFFIterator, f64, f64);
define_iterator_type!(ModelDataIIIterator, u64, u64);
//define_iterator_type_skip!(ModelDataIIIteratorSkip, u64, u64);
//define_iterator_type!(ModelDataFIIterator, f64, u64);
//define_iterator_type!(ModelDataIFIterator, u64, f64);

impl ModelData {
    pub fn iter_float_float(&self) -> ModelDataFFIterator {
        return ModelDataFFIterator::new(&self);
    }
    pub fn iter_int_int(&self) -> ModelDataIIIterator {
        return ModelDataIIIterator::new(&self);
    }

    /*pub fn iter_int_int_skip(&self, factor: usize) -> ModelDataIIIteratorSkip {
        return ModelDataIIIteratorSkip::new(&self, factor);
    }*/
    //pub fn iter_float_int(&self) -> ModelDataFIIterator { return ModelDataFIIterator::new(&self); }
    //pub fn iter_int_float(&self) -> ModelDataIFIterator { return ModelDataIFIterator::new(&self); }

    pub fn empty() -> ModelData {
        return ModelData::FloatKeyToFloatPos(vec![]);
    }

    #[cfg(test)]
    fn into_int_int(self) -> Vec<(u64, u64)> {
        return match self {
            ModelData::FloatKeyToFloatPos(data) => vec_to_ii!(data),
            ModelData::FloatKeyToIntPos(data) => vec_to_ii!(data),
            ModelData::IntKeyToFloatPos(data) => vec_to_ii!(data),
            ModelData::IntKeyToIntPos(data) => data,
        };
    }

    fn as_int_int(&self) -> &[(u64, u64)] {
        return match self {
            ModelData::FloatKeyToFloatPos(_data) => panic!("as_int_int on float/float model data"),
            ModelData::FloatKeyToIntPos(_data) => panic!("as_int_int on float/int model data"),
            ModelData::IntKeyToFloatPos(_data) => panic!("as_int_int on int/float model data"),
            ModelData::IntKeyToIntPos(data) => &data,
        };
    }

    pub fn len(&self) -> usize {
        return match self {
            ModelData::FloatKeyToFloatPos(data) => data.len(),
            ModelData::FloatKeyToIntPos(data) => data.len(),
            ModelData::IntKeyToFloatPos(data) => data.len(),
            ModelData::IntKeyToIntPos(data) => data.len(),
        };
    }

    pub fn get(&self, idx: usize) -> (f64, f64) {
        return match self {
            ModelData::FloatKeyToFloatPos(data) => data[idx],
            ModelData::FloatKeyToIntPos(data) => (data[idx].0, data[idx].1 as f64),
            ModelData::IntKeyToFloatPos(data) => (data[idx].0 as f64, data[idx].1),
            ModelData::IntKeyToIntPos(data) => (data[idx].0 as f64, data[idx].1 as f64),
        };
    }

    pub fn get_key(&self, idx: usize) -> u64 {
        return match self {
            ModelData::FloatKeyToFloatPos(data) => data[idx].0 as u64,
            ModelData::FloatKeyToIntPos(data) => data[idx].0 as u64, 
            ModelData::IntKeyToFloatPos(data) => data[idx].0,
            ModelData::IntKeyToIntPos(data) => data[idx].0
        };
    }
}

pub enum ModelInput {
    Int(u64),
    Float(f64),
}

impl ModelInput {
    fn as_float(&self) -> f64 {
        return match self {
            ModelInput::Int(x) => *x as f64,
            ModelInput::Float(x) => *x,
        };
    }

    fn as_int(&self) -> u64 {
        return match self {
            ModelInput::Int(x) => *x,
            ModelInput::Float(x) => *x as u64,
        };
    }
}

impl From<u64> for ModelInput {
    fn from(i: u64) -> Self {
        ModelInput::Int(i)
    }
}

impl From<f64> for ModelInput {
    fn from(f: f64) -> Self {
        ModelInput::Float(f)
    }
}

pub enum ModelDataType {
    Int,
    Float,
}

impl ModelDataType {
    pub fn c_type(&self) -> &'static str {
        match self {
            ModelDataType::Int => "uint64_t",
            ModelDataType::Float => "double",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModelParam {
    Int(u64),
    Float(f64),
    ShortArray(Vec<u16>),
    IntArray(Vec<u64>),
    Int32Array(Vec<u32>),
    FloatArray(Vec<f64>),
}

impl ModelParam {
    // size in bytes
    pub fn size(&self) -> usize {
        match self {
            ModelParam::Int(_) => 8,
            ModelParam::Float(_) => 8,
            ModelParam::ShortArray(a) => 2 * a.len(),
            ModelParam::IntArray(a) => 8 * a.len(),
            ModelParam::Int32Array(a) => 4 * a.len(),
            ModelParam::FloatArray(a) => 8 * a.len(),
        }
    }

    pub fn c_type(&self) -> &'static str {
        match self {
            ModelParam::Int(_) => "uint64_t",
            ModelParam::Float(_) => "double",
            ModelParam::ShortArray(_) => "short",
            ModelParam::IntArray(_) => "uint64_t",
            ModelParam::Int32Array(_) => "uint32_t",
            ModelParam::FloatArray(_) => "double",
        }
    }

    pub fn is_array(&self) -> bool {
        match self {
            ModelParam::Int(_) => false,
            ModelParam::Float(_) => false,
            ModelParam::ShortArray(_) => true,
            ModelParam::IntArray(_) => true,
            ModelParam::Int32Array(_) => true,
            ModelParam::FloatArray(_) => true
        }
    }

    pub fn c_type_mod(&self) -> &'static str {
        match self {
            ModelParam::Int(_) => "",
            ModelParam::Float(_) => "",
            ModelParam::ShortArray(_) => "[]",
            ModelParam::IntArray(_) => "[]",
            ModelParam::Int32Array(_) => "[]",
            ModelParam::FloatArray(_) => "[]",
        }
    }

    pub fn c_val(&self) -> String {
        match self {
            ModelParam::Int(v) => format!("{}UL", v),
            ModelParam::Float(v) => {
                let mut tmp = format!("{:.}", v);
                if !tmp.contains('.') {
                    tmp.push_str(".0");
                }
                return tmp;
            },
            ModelParam::ShortArray(arr) => {
                let itms: Vec<String> = arr.iter().map(|i| format!("{}", i)).collect();
                return format!("{{ {} }}", itms.join(", "));
            },
            ModelParam::IntArray(arr) => {
                let itms: Vec<String> = arr.iter().map(|i| format!("{}UL", i)).collect();
                return format!("{{ {} }}", itms.join(", "));
            },
            ModelParam::Int32Array(arr) => {
                let itms: Vec<String> = arr.iter().map(|i| format!("{}UL", i)).collect();
                return format!("{{ {} }}", itms.join(", "));
            },
            ModelParam::FloatArray(arr) => {
                let itms: Vec<String> = arr
                    .iter()
                    .map(|i| format!("{:.}", i))
                    .map(|s| if !s.contains('.') { s + ".0" } else { s })
                    .collect();
                return format!("{{ {} }}", itms.join(", "));
            }
        }
    }

    /* useful for debugging floating point issues
    pub fn as_bits(&self) -> u64 {
        return match self {
            ModelParam::Int(v) => *v,
            ModelParam::Float(v) => v.to_bits(),
            ModelParam::ShortArray(_) => panic!("Cannot treat a short array parameter as a float"),
            ModelParam::IntArray(_) => panic!("Cannot treat an int array parameter as a float"),
            ModelParam::FloatArray(_) => panic!("Cannot treat an float array parameter as a float"),
        };
    }*/

    pub fn is_same_type(&self, other: &ModelParam) -> bool {
        return std::mem::discriminant(self) == std::mem::discriminant(other);
    }

    pub fn write_to<T: Write>(&self, target: &mut T) -> Result<(), std::io::Error> {
        match self {
            ModelParam::Int(v) => target.write_u64::<LittleEndian>(*v),
            ModelParam::Float(v) => target.write_f64::<LittleEndian>(*v),
            ModelParam::ShortArray(arr) => {
                for v in arr {
                    target.write_u16::<LittleEndian>(*v)?;
                }

                Ok(())
            },
            
            ModelParam::IntArray(arr) => {
                for v in arr {
                    target.write_u64::<LittleEndian>(*v)?;
                }

                Ok(())
            },

            ModelParam::Int32Array(arr) => {
                for v in arr {
                    target.write_u32::<LittleEndian>(*v)?;
                }

                Ok(())
            },

            ModelParam::FloatArray(arr) => {
                for v in arr {
                    target.write_f64::<LittleEndian>(*v)?;
                }

                Ok(())

            }

        }
    }
    
    pub fn as_float(&self) -> f64 {
        match self {
            ModelParam::Int(v) => *v as f64,
            ModelParam::Float(v) => *v,
            ModelParam::ShortArray(_) => panic!("Cannot treat a short array parameter as a float"),
            ModelParam::IntArray(_) => panic!("Cannot treat an int array parameter as a float"),
            ModelParam::Int32Array(_) => panic!("Cannot treat an int32 array parameter as a float"),
            ModelParam::FloatArray(_) => panic!("Cannot treat an float array parameter as a float"),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ModelParam::Int(_) => 1,
            ModelParam::Float(_) => 1,
            ModelParam::ShortArray(p) => p.len(),
            ModelParam::IntArray(p) => p.len(),
            ModelParam::Int32Array(p) => p.len(),
            ModelParam::FloatArray(p) => p.len()
        }
    }
}

impl From<usize> for ModelParam {
    fn from(i: usize) -> Self {
        ModelParam::Int(i as u64)
    }
}

impl From<u64> for ModelParam {
    fn from(i: u64) -> Self {
        ModelParam::Int(i)
    }
}

impl From<u8> for ModelParam {
    fn from(i: u8) -> Self {
        ModelParam::Int(u64::from(i))
    }
}

impl From<f64> for ModelParam {
    fn from(f: f64) -> Self {
        ModelParam::Float(f)
    }
}

impl From<Vec<u16>> for ModelParam {
    fn from(f: Vec<u16>) -> Self {
        ModelParam::ShortArray(f)
    }
}

impl From<Vec<u64>> for ModelParam {
    fn from(f: Vec<u64>) -> Self {
        ModelParam::IntArray(f)
    }
}

impl From<Vec<u32>> for ModelParam {
    fn from(f: Vec<u32>) -> Self {
        ModelParam::Int32Array(f)
    }
}

impl From<Vec<f64>> for ModelParam {
    fn from(f: Vec<f64>) -> Self {
        ModelParam::FloatArray(f)
    }
}

pub enum ModelRestriction {
    None,
    MustBeTop,
    MustBeBottom,
}

pub trait Model: Sync + Send {
    fn predict_to_float(&self, inp: ModelInput) -> f64 {
        return self.predict_to_int(inp) as f64;
    }

    fn predict_to_int(&self, inp: ModelInput) -> u64 {
        return f64::max(0.0, self.predict_to_float(inp).floor()) as u64;
    }

    fn input_type(&self) -> ModelDataType;
    fn output_type(&self) -> ModelDataType;

    fn params(&self) -> Vec<ModelParam>;

    fn code(&self) -> String;
    fn function_name(&self) -> String;

    fn standard_functions(&self) -> HashSet<StdFunctions> {
        return HashSet::new();
    }

    fn needs_bounds_check(&self) -> bool {
        return true;
    }
    fn restriction(&self) -> ModelRestriction {
        return ModelRestriction::None;
    }
    fn error_bound(&self) -> Option<u64> {
        return None;
    }

    fn set_to_constant_model(&mut self, _constant: u64) -> bool {
        return false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale() {
        let mut v = ModelData::IntKeyToIntPos(vec![(0, 0), (1, 1), (3, 2), (100, 3)]);

        v.scale_targets_to(50, 4);

        let results = v.as_int_int();
        assert_eq!(results[0].1, 0);
        assert_eq!(results[1].1, 12);
        assert_eq!(results[2].1, 25);
        assert_eq!(results[3].1, 37);
    }

    #[test]
    fn test_iter() {
        let data = vec![(0, 1), (1, 2), (3, 3), (100, 4)];

        let v = ModelData::IntKeyToIntPos(data.clone());

        let iterated: Vec<(u64, u64)> = v.iter_int_int().collect();
        assert_eq!(data, iterated);
    }
}
