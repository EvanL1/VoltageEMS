//! Energy Calculator
//!
//! Energy-specific calculations for power management systems:
//! power balance, state of charge, efficiency, load forecast, dispatch optimization.

use crate::error::{ModelError, Result};
use std::collections::HashMap;
use voltage_config::calculations::EnergyCalculation;

/// Energy-specific calculator
pub struct EnergyCalculator;

impl EnergyCalculator {
    /// Create new energy calculator
    pub fn new() -> Self {
        Self
    }

    /// Execute energy calculation
    ///
    /// # Arguments
    /// * `operation` - Energy calculation type
    /// * `values` - Input values keyed by name
    ///
    /// # Returns
    /// JSON result with calculation outputs
    pub fn calculate(
        &self,
        operation: &EnergyCalculation,
        values: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        match operation {
            EnergyCalculation::PowerBalance => self.power_balance(values),
            EnergyCalculation::StateOfCharge => self.state_of_charge(values),
            EnergyCalculation::EnergyEfficiency => self.energy_efficiency(values),
            EnergyCalculation::LoadForecast => self.load_forecast(values),
            EnergyCalculation::OptimalDispatch => self.optimal_dispatch(values),
            EnergyCalculation::CostOptimization => self.cost_optimization(values),
            #[allow(unreachable_patterns)]
            _ => Err(ModelError::calculation(format!(
                "Energy calculation type not yet implemented: {:?}",
                operation
            ))),
        }
    }

    /// Calculate power balance
    ///
    /// Inputs: pv_power, battery_power, load_power, grid_power
    fn power_balance(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let pv = values.get("pv_power").unwrap_or(&0.0);
        let battery = values.get("battery_power").unwrap_or(&0.0);
        let load = values.get("load_power").unwrap_or(&0.0);
        let grid = values.get("grid_power").unwrap_or(&0.0);

        // Power balance: Sources (pv + battery) - Consumption (load + grid)
        let balance = pv + battery - load - grid;

        Ok(serde_json::json!({
            "power_balance": balance,
            "is_balanced": balance.abs() < 0.001,
            "components": {
                "pv": pv,
                "battery": battery,
                "load": load,
                "grid": grid
            }
        }))
    }

    /// Calculate state of charge using coulomb counting
    ///
    /// Inputs: battery_current, battery_voltage, battery_capacity, previous_soc, time_delta
    fn state_of_charge(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let current = values.get("battery_current").unwrap_or(&0.0);
        let voltage = values.get("battery_voltage").unwrap_or(&0.0);
        let capacity = values.get("battery_capacity").unwrap_or(&100.0);
        let previous_soc = values.get("previous_soc").unwrap_or(&50.0);
        let dt = values.get("time_delta").unwrap_or(&1.0); // seconds

        // Coulomb counting method
        let charge_change = current * dt / 3600.0; // Convert to Ah
        let soc_change = (charge_change / capacity) * 100.0;
        let new_soc = (previous_soc + soc_change).clamp(0.0, 100.0);

        Ok(serde_json::json!({
            "soc": new_soc,
            "soc_change": soc_change,
            "energy_stored": new_soc * capacity * voltage / 100.0,
            "power": current * voltage
        }))
    }

    /// Calculate energy efficiency
    ///
    /// Inputs: input_power, output_power
    fn energy_efficiency(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let input_power = values.get("input_power").unwrap_or(&0.0);
        let output_power = values.get("output_power").unwrap_or(&0.0);

        let efficiency = if *input_power > 0.0 {
            (output_power / input_power * 100.0).min(100.0)
        } else {
            0.0
        };

        let losses = input_power - output_power;

        Ok(serde_json::json!({
            "efficiency_percent": efficiency,
            "losses_watts": losses,
            "input_power": input_power,
            "output_power": output_power
        }))
    }

    /// Simple load forecast based on historical average
    ///
    /// Inputs: current_load, avg_load_24h, peak_load_24h, hour_of_day
    fn load_forecast(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let current_load = values.get("current_load").unwrap_or(&0.0);
        let avg_load = values.get("avg_load_24h").unwrap_or(&0.0);
        let peak_load = values.get("peak_load_24h").unwrap_or(&0.0);
        let hour_of_day = values.get("hour_of_day").unwrap_or(&12.0);

        // Simple time-based forecast
        let hour_factor = 1.0 + 0.3 * ((hour_of_day - 12.0).abs() / 12.0 - 0.5);
        let forecast = avg_load * hour_factor;

        Ok(serde_json::json!({
            "forecast_load": forecast,
            "current_load": current_load,
            "confidence": 0.75,
            "peak_probability": if *peak_load > 0.0 { forecast / peak_load } else { 0.0 }
        }))
    }

    /// Optimal dispatch calculation
    ///
    /// Inputs: pv_available, battery_available, grid_price, load_demand, battery_soc
    fn optimal_dispatch(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let pv_available = values.get("pv_available").unwrap_or(&0.0);
        let battery_available = values.get("battery_available").unwrap_or(&0.0);
        let grid_price = values.get("grid_price").unwrap_or(&0.1);
        let load_demand = values.get("load_demand").unwrap_or(&0.0);
        let battery_soc = values.get("battery_soc").unwrap_or(&50.0);

        // Simple dispatch logic
        let pv_dispatch = pv_available.min(*load_demand);
        let mut battery_dispatch = 0.0;
        let mut grid_dispatch = 0.0;

        let remaining = load_demand - pv_dispatch;

        if remaining > 0.0 {
            // Use battery if SOC > 20% and grid price is high
            if *battery_soc > 20.0 && *grid_price > 0.15 {
                battery_dispatch = remaining.min(*battery_available);
                grid_dispatch = remaining - battery_dispatch;
            } else {
                grid_dispatch = remaining;
            }
        }

        Ok(serde_json::json!({
            "dispatch": {
                "pv": pv_dispatch,
                "battery": battery_dispatch,
                "grid": grid_dispatch
            },
            "total_cost": grid_dispatch * grid_price,
            "renewable_ratio": if *load_demand > 0.0 {
                (pv_dispatch + battery_dispatch) / load_demand
            } else {
                0.0
            }
        }))
    }

    /// Cost optimization calculation
    ///
    /// Inputs: energy_consumed, peak_demand, energy_rate, demand_rate, solar_generated, solar_credit_rate
    fn cost_optimization(&self, values: &HashMap<String, f64>) -> Result<serde_json::Value> {
        let energy_consumed = values.get("energy_consumed").unwrap_or(&0.0);
        let peak_demand = values.get("peak_demand").unwrap_or(&0.0);
        let energy_rate = values.get("energy_rate").unwrap_or(&0.1);
        let demand_rate = values.get("demand_rate").unwrap_or(&10.0);
        let solar_generated = values.get("solar_generated").unwrap_or(&0.0);
        let solar_credit_rate = values.get("solar_credit_rate").unwrap_or(&0.08);

        let energy_cost = energy_consumed * energy_rate;
        let demand_cost = peak_demand * demand_rate;
        let solar_credit = solar_generated * solar_credit_rate;
        let total_cost = energy_cost + demand_cost - solar_credit;

        Ok(serde_json::json!({
            "energy_cost": energy_cost,
            "demand_cost": demand_cost,
            "solar_credit": solar_credit,
            "total_cost": total_cost,
            "cost_per_kwh": if *energy_consumed > 0.0 {
                total_cost / energy_consumed
            } else {
                0.0
            }
        }))
    }
}

impl Default for EnergyCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_balance() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("pv_power".to_string(), 5000.0);
        values.insert("battery_power".to_string(), 2000.0);
        values.insert("load_power".to_string(), 6000.0);
        values.insert("grid_power".to_string(), -1000.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();

        let balance = result["power_balance"].as_f64().unwrap();
        // 5000 + 2000 - 6000 - (-1000) = 2000
        assert_eq!(balance, 2000.0);
    }

    #[test]
    fn test_power_balance_perfect() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("pv_power".to_string(), 1000.0);
        values.insert("battery_power".to_string(), 0.0);
        values.insert("load_power".to_string(), 1000.0);
        values.insert("grid_power".to_string(), 0.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();

        assert!(result["is_balanced"].as_bool().unwrap());
    }

    #[test]
    fn test_state_of_charge_charging() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("battery_current".to_string(), 10.0);
        values.insert("battery_voltage".to_string(), 48.0);
        values.insert("battery_capacity".to_string(), 100.0);
        values.insert("previous_soc".to_string(), 50.0);
        values.insert("time_delta".to_string(), 3600.0); // 1 hour

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        // 50% + (10A * 1h / 100Ah) * 100 = 60%
        assert!((soc - 60.0).abs() < 0.1);
    }

    #[test]
    fn test_state_of_charge_clamping() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("battery_current".to_string(), 50.0);
        values.insert("battery_capacity".to_string(), 100.0);
        values.insert("previous_soc".to_string(), 95.0);
        values.insert("time_delta".to_string(), 3600.0);

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        assert_eq!(soc, 100.0); // Clamped to 100%
    }

    #[test]
    fn test_energy_efficiency() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 1000.0);
        values.insert("output_power".to_string(), 950.0);

        let result = calculator
            .calculate(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();

        assert_eq!(result["efficiency_percent"].as_f64().unwrap(), 95.0);
        assert_eq!(result["losses_watts"].as_f64().unwrap(), 50.0);
    }

    #[test]
    fn test_efficiency_zero_input() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 0.0);
        values.insert("output_power".to_string(), 100.0);

        let result = calculator
            .calculate(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();

        assert_eq!(result["efficiency_percent"].as_f64().unwrap(), 0.0);
    }
}
