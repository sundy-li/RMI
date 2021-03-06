use crate::models::*;
use crate::train;
use crate::codegen;
use log::*;
use json::*;
use indicatif::{ProgressBar};
use rayon::prelude::*;
use std::collections::BTreeSet;
use tabular::{Table, row};

const TOP_ONLY_LAYERS: &[&str] = &["radix", "radix18", "radix22", "robust_linear"];
const ANYWHERE_LAYERS: &[&str] = &["linear", "cubic", "linear_spline"];
//const SPECIALTY_TOP_LAYERS: &[&str] = &["histogram", "loglinear", "normal", "lognormal", "bradix"];

fn get_branching_factors() -> Vec<u64> {
    let mut branching_factors: Vec<u64> = Vec::new();
    for i in 6..25 {
        branching_factors.push((2 as u64).pow(i));
    }
    return branching_factors;
}

fn pareto_front(results: &[RMIStatistics]) -> Vec<RMIStatistics> {
    let mut on_front: Vec<RMIStatistics> = Vec::new();

    for result in results.iter() {
        if results.iter().any(|v| result.dominated_by(v)) {
            // not on the front
            continue;
        }

        on_front.push(result.clone());
    }

    return on_front;
}

fn narrow_front(results: &[RMIStatistics], desired_size: usize) -> Vec<RMIStatistics> {
    assert!(desired_size >= 2);
    if results.len() <= desired_size {
        return results.to_vec();
    }

    let mut tmp = results.to_vec();
    tmp.sort_by(
        |a, b| a.size.partial_cmp(&b.size).unwrap()
    );

    let best_mod = tmp.remove(0);
    while tmp.len() > desired_size - 1 {
        // find the two items closest in size and remove less accuracte one.
        let smallest_gap =
            (0..tmp.len()-1).zip(1..tmp.len())
            .map(|(idx1, idx2)| (idx1, idx2,
                                 (tmp[idx2].size as f64) / (tmp[idx1].size as f64)))
            .min_by(|(_, _, v1), (_, _, v2)| v1.partial_cmp(v2).unwrap()).unwrap();

        let err1 = tmp[smallest_gap.0].average_log2_error;
        let err2 = tmp[smallest_gap.1].average_log2_error;
        if err1 > err2 {
            tmp.remove(smallest_gap.0);
        } else {
            tmp.remove(smallest_gap.0);
        }
    }
    tmp.insert(0, best_mod);

    return tmp;

    

}

fn first_phase_configs() -> Vec<(String, u64)> {
    let mut results = Vec::new();
    let mut all_top_models = Vec::new();
    all_top_models.extend_from_slice(TOP_ONLY_LAYERS);
    all_top_models.extend_from_slice(ANYWHERE_LAYERS);
    
    for top_model in all_top_models {
        for bottom_model in ANYWHERE_LAYERS {
            for branching_factor in get_branching_factors().iter().step_by(5) {
                results.push((format!("{},{}", top_model, bottom_model), *branching_factor));
            }
        }
    }

    return results;
}

fn second_phase_configs(first_phase: &[RMIStatistics]) -> Vec<(String, u64)> {
    let qualifying_model_configs = {
        let on_front = pareto_front(first_phase);
        let mut qualifying = BTreeSet::new();
        for result in on_front {
            qualifying.insert(result.models.clone());
        }
        qualifying
    };

    info!("Qualifying model types for phase 2: {:?}", qualifying_model_configs);
    let mut results = Vec::new();

    for model in qualifying_model_configs.iter() {
        for branching_factor in get_branching_factors() {
            if first_phase.iter().any(|v| v.has_config(&model, branching_factor)) {
                continue;
            }

            results.push((model.clone(), branching_factor));
        }
    }
    
    return results;
}

#[derive(Clone, Debug)]
pub struct RMIStatistics {
    pub models: String,
    pub branching_factor: u64,
    pub average_log2_error: f64,
    pub max_log2_error: f64,
    pub size: u64
}

impl RMIStatistics {
    fn from_trained(rmi: &train::TrainedRMI) -> RMIStatistics {
        return RMIStatistics {
            average_log2_error: rmi.model_avg_log2_error,
            max_log2_error: rmi.model_max_log2_error,
            size: codegen::rmi_size(&rmi.rmi, true),
            models: rmi.models.clone(),
            branching_factor: rmi.branching_factor
        };
    }

    fn dominated_by(&self, other: &RMIStatistics) -> bool {
        if self.size < other.size { return false; }
        if self.average_log2_error < other.average_log2_error { return false; }

        if self.size == other.size && self.average_log2_error <= other.average_log2_error {
            return false;
        }

        if self.size <= other.size && self.average_log2_error == other.average_log2_error {
            return false;
        }

        return true;
    }

    fn has_config(&self, models: &str, branching_factor: u64) -> bool {
        return self.models == models && self.branching_factor == branching_factor;
    }

    pub fn display_table(itms: &[RMIStatistics]) {
        let mut table = Table::new("{:<} {:>} {:>} {:>} {:>}");
        table.add_row(row!("Models", "Branch", "   AvgLg2",
                           "   MaxLg2", "   Size (b)"));
        for itm in itms {
            table.add_row(row!(itm.models.clone(),
                               format!("{:10}", itm.branching_factor),
                               format!("     {:2.5}", itm.average_log2_error),
                               format!("     {:2.5}", itm.max_log2_error),
                               format!("     {}", itm.size)));
        }

        print!("{}", table);
    }
    
    pub fn to_grid_spec(&self, namespace: &str) -> JsonValue {
        return object!(
            "layers" => self.models.clone(),
            "branching factor" => self.branching_factor,
            "namespace" => namespace,
            "size" => self.size,
            "average log2 error" => self.average_log2_error,
            "binary" => true
        );
    }
}

fn measure_rmis(data: &ModelData, configs: &[(String, u64)]) -> Vec<RMIStatistics> {
    let pbar = ProgressBar::new(configs.len() as u64);
    configs.par_iter()
        .map(|(models, branch_factor)| {
            let mut md = ModelDataWrapper::new(data);
            let res = train::train(&mut md, models, *branch_factor);
            pbar.inc(1);
            RMIStatistics::from_trained(&res)
        }).collect()
}

pub fn find_pareto_efficient_configs(data: &ModelData, restrict: usize)
                                     -> Vec<RMIStatistics>{
    let initial_configs  = first_phase_configs();
    let first_phase_results = measure_rmis(data, &initial_configs);

    let next_configs = second_phase_configs(&first_phase_results);
    let second_phase_results = measure_rmis(data, &next_configs);
    
    let mut final_front = pareto_front(&second_phase_results);
    final_front = narrow_front(&final_front, restrict);
    final_front.sort_by(
        |a, b| a.average_log2_error.partial_cmp(&b.average_log2_error).unwrap()
    );

    return final_front;
}
