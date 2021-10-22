use super::{IntegerChip, IntegerInstructions};
use crate::circuit::range::{RangeInstructions, RangeTune};
use crate::circuit::{AssignedInteger, AssignedValue};
use crate::rns::Quotient;
use crate::NUMBER_OF_LIMBS;

use halo2::arithmetic::FieldExt;
use halo2::circuit::{Cell, Region};
use halo2::plonk::Error;

impl<W: FieldExt, N: FieldExt> IntegerChip<W, N> {
    fn mul_v0_range_tune(&self) -> RangeTune {
        RangeTune::Overflow(2)
    }

    fn mul_v1_range_tune(&self) -> RangeTune {
        RangeTune::Overflow(3)
    }

    fn mul_quotient_range_tune(&self) -> RangeTune {
        // TODO:
        RangeTune::Fits
    }

    fn mul_result_range_tune(&self) -> RangeTune {
        // TODO:
        RangeTune::Fits
    }

    pub(crate) fn _mul(&self, region: &mut Region<'_, N>, a: &mut AssignedInteger<N>, b: &mut AssignedInteger<N>) -> Result<AssignedInteger<N>, Error> {
        let main_gate = self.main_gate_config();
        let mut offset = 0;
        let negative_wrong_modulus: Vec<N> = self.rns.negative_wrong_modulus.limbs();

        let reduction_result = a.integer().map(|integer_a| {
            let b_integer = b.integer().unwrap();
            self.rns.mul(&integer_a, &b_integer)
        });

        let quotient = reduction_result.as_ref().map(|reduction_result| {
            let quotient = match reduction_result.quotient.clone() {
                Quotient::Long(quotient) => quotient,
                _ => panic!("long quotient expected"),
            };
            quotient
        });

        let result = reduction_result.as_ref().map(|u| u.result.clone());
        let result = &mut self.assign_integer(region, result, &mut offset)?;
        let quotient = &mut self.assign_integer(region, quotient, &mut offset)?;
        let intermediate_values: Option<Vec<N>> = reduction_result.as_ref().map(|u| u.t.iter().map(|t| t.fe()).collect());

        let u_0 = reduction_result.as_ref().map(|u| u.u_0);
        let v_0 = reduction_result.as_ref().map(|u| u.v_0);
        let u_1 = reduction_result.as_ref().map(|u| u.u_1);
        let v_1 = reduction_result.as_ref().map(|u| u.v_1);

        // t_0 = a_0 * b_0 + q_0 * p_0

        // t_1 =    a_0 * b_1 + a_1 * b_0 + q_0 * p_1 + q_1 * p_0
        // t_1 =    a_0 * b_1 + q_0 * p_1 + tmp
        // tmp =    a_1 * b_0 + q_1 * p_0

        // t_2   =    a_0 * b_2 + a_1 * b_1e + a_2 * b_0 + q_0 * p_2 + q_1 * p_1 + q_2 * p_0
        // t_2   =    a_0 * b_2 + q_0 * p_2 + tmp_a
        // tmp_a =    a_1 * b_1 + q_1 * p_1 + tmp_b
        // tmp_b =    a_2 * b_0 + q_2 * p_0

        // t_3   =    a_0 * b_3 + a_1 * b_2 + a_1 * b_2 + a_3 * b_0 + q_0 * p_3 + q_1 * p_2 + q_2 * p_1 + q_3 * p_0
        // t_3   =    a_0 * b_3 + q_0 * p_3 + tmp_a
        // tmp_a =    a_1 * b_2 + q_1 * p_2 + tmp_b
        // tmp_b =    a_2 * b_1 + q_2 * p_1 + tmp_c
        // tmp_c =    a_3 * b_0 + q_3 * p_0

        // | A   | B   | C   | D     |
        // | --- | --- | --- | ----- |
        // | a_0 | b_0 | q_0 | t_0   |

        // | a_0 | b_1 | q_1 | t_1   |
        // | a_1 | b_0 | q_0 | tmp   |

        // | a_0 | b_2 | q_2 | t_2   |
        // | a_1 | b_1 | q_1 | tmp_a |
        // | a_2 | b_0 | q_0 | tmp_b |

        // | a_0 | b_3 | q_3 | t_3   |
        // | a_1 | b_1 | q_2 | tmp_b |
        // | a_2 | b_2 | q_1 | tmp_a |
        // | a_3 | b_0 | q_0 | tmp_c |

        let mut intermediate_values_cycling: Vec<Cell> = vec![];

        for i in 0..NUMBER_OF_LIMBS {
            let mut t = intermediate_values.as_ref().map(|intermediate_values| intermediate_values[i]);

            for j in 0..=i {
                let k = i - j;

                let a_j_new_cell = region.assign_advice(|| "a_", main_gate.a, offset, || a.limb_value(j))?;
                let b_k_new_cell = region.assign_advice(|| "b_", main_gate.b, offset, || b.limb_value(k))?;
                let q_k_new_cell = region.assign_advice(|| "q_", main_gate.c, offset, || quotient.limb_value(k))?;
                let t_i_cell = region.assign_advice(|| "t_", main_gate.d, offset, || Ok(t.ok_or(Error::SynthesisError)?.clone()))?;

                region.assign_fixed(|| "s_m", main_gate.s_mul, offset, || Ok(N::one()))?;
                region.assign_fixed(|| "s_c", main_gate.sc, offset, || Ok(negative_wrong_modulus[j]))?;
                region.assign_fixed(|| "s_d", main_gate.sd, offset, || Ok(-N::one()))?;

                if k == 0 {
                    region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(N::zero()))?;
                } else {
                    region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(N::one()))?;
                }

                // zero selectors
                region.assign_fixed(|| "s_a", main_gate.sa, offset, || Ok(N::zero()))?;
                region.assign_fixed(|| "s_b", main_gate.sb, offset, || Ok(N::zero()))?;
                region.assign_fixed(|| "s_constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

                // cycle and update operand limb assignments

                a.cycle_cell(region, j, a_j_new_cell)?;
                b.cycle_cell(region, k, b_k_new_cell)?;
                quotient.cycle_cell(region, k, q_k_new_cell)?;

                if j == 0 {
                    // first time we see t_j assignment
                    intermediate_values_cycling.push(t_i_cell);
                }

                // update running temp value
                t = t.map(|t| {
                    let a = a.limb_value(j).unwrap();
                    let b = b.limb_value(k).unwrap();
                    let q = quotient.limb_value(k).unwrap();
                    let p = negative_wrong_modulus[j];
                    t - (a * b + q * p)
                });

                offset += 1;
            }
        }

        // u_0 = t_0 + (t_1 * R) - r_0 - (r_1 * R)
        // u_0 = v_0 * R^2

        // | A   | B   | C   | D     |
        // | --- | --- | --- | ----- |
        // | t_0 | t_1 | r_0 | r_1   |
        // | -   | -   | v_0 | u_0   |

        let left_shifter_r = self.rns.left_shifter_r;
        let left_shifter_2r = self.rns.left_shifter_2r;

        let t_0_new_cell = region.assign_advice(
            || "t_0",
            main_gate.a,
            offset,
            || Ok(intermediate_values.as_ref().ok_or(Error::SynthesisError)?[0]),
        )?;
        let t_1_new_cell = region.assign_advice(
            || "t_1",
            main_gate.b,
            offset,
            || Ok(intermediate_values.as_ref().ok_or(Error::SynthesisError)?[1]),
        )?;

        let r_0_new_cell = region.assign_advice(|| "r_0", main_gate.c, offset, || result.limb_value(0))?;
        let r_1_new_cell = region.assign_advice(|| "r_1", main_gate.d, offset, || result.limb_value(1))?;

        region.assign_fixed(|| "s_a", main_gate.sa, offset, || Ok(N::one()))?;
        region.assign_fixed(|| "s_b", main_gate.sb, offset, || Ok(left_shifter_r))?;
        region.assign_fixed(|| "s_c", main_gate.sc, offset, || Ok(-N::one()))?;
        region.assign_fixed(|| "s_d", main_gate.sd, offset, || Ok(-left_shifter_r))?;
        region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(-N::one()))?;

        region.assign_fixed(|| "s_m", main_gate.s_mul, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

        region.constrain_equal(intermediate_values_cycling[0], t_0_new_cell)?;
        region.constrain_equal(intermediate_values_cycling[1], t_1_new_cell)?;

        result.cycle_cell(region, 0, r_0_new_cell)?;
        result.cycle_cell(region, 1, r_1_new_cell)?;

        offset += 1;

        let _ = region.assign_advice(|| "u_0", main_gate.d, offset, || u_0.ok_or(Error::SynthesisError))?;
        let v_0_cell = region.assign_advice(|| "v_0", main_gate.c, offset, || v_0.ok_or(Error::SynthesisError))?;

        region.assign_fixed(|| "s_c", main_gate.sc, offset, || Ok(left_shifter_2r))?;
        region.assign_fixed(|| "s_d", main_gate.sd, offset, || Ok(-N::one()))?;

        region.assign_fixed(|| "s_a", main_gate.sa, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_b", main_gate.sb, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_m", main_gate.s_mul, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

        offset += 1;

        // u_1 = t_2 + (t_3 * R) - r_2 - (r_3 * R)
        // v_1 * 2R = u_1 + v_0

        // | A   | B   | C   | D     |
        // | --- | --- | --- | ----- |
        // | t_2 | t_3 | r_2 | r_3   |
        // | -   | v_1 | v_0 | u_1   |

        let t_2_new_cell = region.assign_advice(
            || "t_2",
            main_gate.a,
            offset,
            || Ok(intermediate_values.as_ref().ok_or(Error::SynthesisError)?[2]),
        )?;
        let t_3_new_cell = region.assign_advice(
            || "t_3",
            main_gate.b,
            offset,
            || Ok(intermediate_values.as_ref().ok_or(Error::SynthesisError)?[3]),
        )?;

        let r_2_new_cell = region.assign_advice(|| "r_0", main_gate.c, offset, || result.limb_value(2))?;
        let r_3_new_cell = region.assign_advice(|| "r_1", main_gate.d, offset, || result.limb_value(3))?;

        region.assign_fixed(|| "s_a", main_gate.sa, offset, || Ok(N::one()))?;
        region.assign_fixed(|| "s_b", main_gate.sb, offset, || Ok(left_shifter_r))?;
        region.assign_fixed(|| "s_c", main_gate.sc, offset, || Ok(-N::one()))?;
        region.assign_fixed(|| "s_d", main_gate.sd, offset, || Ok(-left_shifter_r))?;
        region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(-N::one()))?;

        region.assign_fixed(|| "s_m", main_gate.s_mul, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

        region.constrain_equal(intermediate_values_cycling[2], t_2_new_cell)?;
        region.constrain_equal(intermediate_values_cycling[3], t_3_new_cell)?;

        result.cycle_cell(region, 2, r_2_new_cell)?;
        result.cycle_cell(region, 3, r_3_new_cell)?;

        offset += 1;

        let v_1_cell = region.assign_advice(|| "v_1", main_gate.b, offset, || v_1.ok_or(Error::SynthesisError))?;
        let v_0_new_cell = region.assign_advice(|| "v_0", main_gate.c, offset, || v_0.ok_or(Error::SynthesisError))?;
        let _ = region.assign_advice(|| "u_1", main_gate.d, offset, || u_1.ok_or(Error::SynthesisError))?;

        region.assign_fixed(|| "s_b", main_gate.sb, offset, || Ok(left_shifter_2r))?;
        region.assign_fixed(|| "s_c", main_gate.sc, offset, || Ok(-N::one()))?;
        region.assign_fixed(|| "s_d", main_gate.sd, offset, || Ok(-N::one()))?;

        region.assign_fixed(|| "s_a", main_gate.sa, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_m", main_gate.s_mul, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_d_next", main_gate.sd_next, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "s_constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

        region.constrain_equal(v_0_cell, v_0_new_cell)?;

        let v_0 = &mut AssignedValue::<N>::new(v_0_new_cell, v_0);
        let v_1 = &mut AssignedValue::<N>::new(v_1_cell, v_1);

        offset += 1;

        // ranges

        let range_chip = self.range_chip();

        range_chip.range_integer(region, quotient, self.mul_quotient_range_tune(), &mut offset)?;
        range_chip.range_integer(region, result, self.mul_result_range_tune(), &mut offset)?;
        let _ = range_chip.range_value(region, v_0, self.mul_v0_range_tune(), &mut offset)?;
        let _ = range_chip.range_value(region, v_1, self.mul_v1_range_tune(), &mut offset)?;

        let a_native_new_cell = region.assign_advice(|| "a", main_gate.a, offset, || a.native_value())?;
        let b_native_new_cell = region.assign_advice(|| "b", main_gate.b, offset, || b.native_value())?;
        let q_native_new_cell = region.assign_advice(|| "d", main_gate.c, offset, || quotient.native_value())?;
        let r_native_new_cell = region.assign_advice(|| "c", main_gate.d, offset, || result.native_value())?;

        region.assign_fixed(|| "a * b", main_gate.s_mul, offset, || Ok(-N::one()))?;
        region.assign_fixed(|| "c", main_gate.sc, offset, || Ok(self.rns.wrong_modulus_in_native_modulus))?;
        region.assign_fixed(|| "d", main_gate.sd, offset, || Ok(N::one()))?;

        region.assign_fixed(|| "a", main_gate.sa, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "b", main_gate.sb, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "d_next", main_gate.sd_next, offset, || Ok(N::zero()))?;
        region.assign_fixed(|| "constant", main_gate.s_constant, offset, || Ok(N::zero()))?;

        a.cycle_native_cell(region, a_native_new_cell)?;
        b.cycle_native_cell(region, b_native_new_cell)?;
        result.cycle_native_cell(region, r_native_new_cell)?;
        quotient.cycle_native_cell(region, q_native_new_cell)?;

        Ok(result.clone())
    }
}
