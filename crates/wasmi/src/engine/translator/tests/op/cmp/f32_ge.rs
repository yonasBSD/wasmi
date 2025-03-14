use super::*;

const WASM_OP: WasmOp = WasmOp::cmp(WasmType::F32, "ge");

#[test]
#[cfg_attr(miri, ignore)]
fn same_reg() {
    let expected = [
        Instruction::f32_le(Reg::from(1), Reg::from(0), Reg::from(0)),
        Instruction::return_reg(1),
    ];
    test_binary_same_reg(WASM_OP, expected)
}

#[test]
#[cfg_attr(miri, ignore)]
fn reg_reg() {
    test_binary_reg_reg(WASM_OP, swap_ops!(Instruction::f32_le))
}

#[test]
#[cfg_attr(miri, ignore)]
fn reg_imm() {
    test_binary_reg_imm32(WASM_OP, 1.0_f32, swap_ops!(Instruction::f32_le))
}

#[test]
#[cfg_attr(miri, ignore)]
fn reg_imm_lhs() {
    test_binary_reg_imm32_lhs(WASM_OP, 1.0_f32, swap_ops!(Instruction::f32_le))
}

#[test]
#[cfg_attr(miri, ignore)]
fn reg_nan() {
    test_binary_reg_imm_with(WASM_OP, f32::NAN, [Instruction::return_imm32(false)]).run()
}

#[test]
#[cfg_attr(miri, ignore)]
fn nan_reg() {
    test_binary_reg_imm_lhs_with(WASM_OP, f32::NAN, [Instruction::return_imm32(false)]).run()
}

#[test]
#[cfg_attr(miri, ignore)]
fn consteval() {
    fn test_with(lhs: f32, rhs: f32, result: bool) {
        test_binary_consteval(
            WASM_OP,
            lhs,
            rhs,
            [Instruction::ReturnImm32 {
                value: AnyConst32::from(result),
            }],
        );
    }
    test_with(1.0, 1.0, true);
    test_with(1.0, 2.0, false);
    test_with(2.0, 1.0, true);
}
