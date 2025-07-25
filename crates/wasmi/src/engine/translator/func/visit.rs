use super::{
    control_frame::{
        BlockControlFrame,
        BlockHeight,
        ControlFrame,
        ControlFrameBase,
        IfControlFrame,
        IfReachability,
        LoopControlFrame,
        UnreachableControlFrame,
    },
    stack::TypedProvider,
    FuncTranslator,
    TypedVal,
};
use crate::{
    core::{wasm, FuelCostsProvider, TrapCode, ValType, F32, F64},
    engine::{
        translator::func::{AcquiredTarget, Provider},
        BlockType,
    },
    ir::{
        self,
        index::{self, FuncType},
        BoundedRegSpan,
        Const16,
        Instruction,
        Reg,
    },
    module::{self, FuncIdx, MemoryIdx, TableIdx, WasmiValueType},
    Error,
    ExternRef,
    FuncRef,
    Mutability,
};
use wasmparser::VisitOperator;

/// Used to swap operands of binary [`Instruction`] constructor.
macro_rules! swap_ops {
    ($fn_name:path) => {
        |result: Reg, lhs, rhs| -> Instruction { $fn_name(result, rhs, lhs) }
    };
}

macro_rules! impl_visit_operator {
    ( @mvp $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @sign_extension $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @saturating_float_to_int $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @bulk_memory $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @reference_types $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @tail_call $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @wide_arithmetic $($rest:tt)* ) => {
        impl_visit_operator!(@@skipped $($rest)*);
    };
    ( @@skipped $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident $_ann:tt $($rest:tt)* ) => {
        // We skip Wasm operators that we already implement manually.
        impl_visit_operator!($($rest)*);
    };
    ( @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident $_ann:tt $($rest:tt)* ) => {
        // Wildcard match arm for all the other (yet) unsupported Wasm proposals.
        fn $visit(&mut self $($(, $arg: $argty)*)?) -> Self::Output {
            self.translate_unsupported_operator(stringify!($op))
        }
        impl_visit_operator!($($rest)*);
    };
    () => {};
}

impl<'a> VisitOperator<'a> for FuncTranslator {
    type Output = Result<(), Error>;

    #[cfg(feature = "simd")]
    fn simd_visitor(
        &mut self,
    ) -> Option<&mut dyn wasmparser::VisitSimdOperator<'a, Output = Self::Output>> {
        Some(self)
    }

    wasmparser::for_each_visit_operator!(impl_visit_operator);

    fn visit_unreachable(&mut self) -> Self::Output {
        bail_unreachable!(self);
        self.push_base_instr(Instruction::trap(TrapCode::UnreachableCodeReached))?;
        self.reachable = false;
        Ok(())
    }

    fn visit_nop(&mut self) -> Self::Output {
        // Nothing to do for Wasm `nop` instructions.
        Ok(())
    }

    fn visit_block(&mut self, block_type: wasmparser::BlockType) -> Self::Output {
        let block_type = BlockType::new(block_type, &self.module);
        if !self.is_reachable() {
            // We keep track of unreachable control flow frames so that we
            // can associated `end` operators to their respective control flow
            // frames and precisely know when the code is reachable again.
            self.control_stack
                .push_frame(UnreachableControlFrame::Block);
            return Ok(());
        }
        self.preserve_locals()?;
        // Inherit [`Instruction::ConsumeFuel`] from parent control frame.
        //
        // # Note
        //
        // This is an optimization to reduce the number of [`Instruction::ConsumeFuel`]
        // and is applicable since Wasm `block` are entered unconditionally.
        let fuel_instr = self.fuel_instr();
        let stack_height = BlockHeight::new(self.engine(), self.stack.height(), block_type)?;
        let end_label = self.instr_encoder.new_label();
        let len_block_params = block_type.len_params(self.engine());
        let len_branch_params = block_type.len_results(self.engine());
        let branch_params = self.alloc_branch_params(len_block_params, len_branch_params)?;
        self.control_stack.push_frame(BlockControlFrame::new(
            block_type,
            end_label,
            branch_params,
            stack_height,
            fuel_instr,
        ));
        Ok(())
    }

    fn visit_loop(&mut self, block_type: wasmparser::BlockType) -> Self::Output {
        let block_type = BlockType::new(block_type, &self.module);
        if !self.is_reachable() {
            // See `visit_block` for rational of tracking unreachable control flow.
            self.control_stack.push_frame(UnreachableControlFrame::Loop);
            return Ok(());
        }
        self.preserve_locals()?;
        // Copy `loop` parameters over to where it expects its branch parameters.
        let len_block_params = block_type.len_params(self.engine());
        self.stack
            .pop_n(usize::from(len_block_params), &mut self.providers);
        let branch_params = self.stack.push_dynamic_n(usize::from(len_block_params))?;
        let fuel_info = self.fuel_info();
        self.instr_encoder.encode_copies(
            &mut self.stack,
            BoundedRegSpan::new(branch_params, len_block_params),
            &self.providers[..],
            &fuel_info,
        )?;
        self.instr_encoder.reset_last_instr();
        // Create loop header label and immediately pin it.
        let header = self.instr_encoder.new_label();
        self.instr_encoder.pin_label(header);
        // Optionally create the loop's [`Instruction::ConsumeFuel`].
        //
        // This is handling the fuel required for a single iteration of the loop.
        //
        // Note: The fuel instruction for the loop must be encoded after the loop header is
        //       pinned so that loop iterations will properly consume fuel per iteration.
        let consume_fuel = self.make_fuel_instr()?;
        // Finally create the loop control frame.
        self.control_stack.push_frame(LoopControlFrame::new(
            block_type,
            header,
            branch_params,
            consume_fuel,
        ));
        Ok(())
    }

    fn visit_if(&mut self, block_type: wasmparser::BlockType) -> Self::Output {
        let block_type = BlockType::new(block_type, &self.module);
        if !self.is_reachable() {
            // We keep track of unreachable control flow frames so that we
            // can associated `end` operators to their respective control flow
            // frames and precisely know when the code is reachable again.
            self.control_stack.push_frame(UnreachableControlFrame::If);
            return Ok(());
        }
        let condition = self.stack.pop();
        let stack_height = BlockHeight::new(self.engine(), self.stack.height(), block_type)?;
        self.preserve_locals()?;
        let end_label = self.instr_encoder.new_label();
        let len_block_params = block_type.len_params(self.engine());
        let len_branch_params = block_type.len_results(self.engine());
        let branch_params = self.alloc_branch_params(len_block_params, len_branch_params)?;
        let (reachability, fuel_instr) = match condition {
            TypedProvider::Const(condition) => {
                // Case: the `if` condition is a constant value and
                //       therefore it is known upfront which branch
                //       it will take.
                //       Furthermore the non-taken branch is known
                //       to be unreachable code.
                let reachability = match i32::from(condition) != 0 {
                    true => IfReachability::OnlyThen,
                    false => {
                        // We know that the `then` block is unreachable therefore
                        // we update the reachability until we see the `else` branch.
                        self.reachable = false;
                        IfReachability::OnlyElse
                    }
                };
                // An `if` control frame with a constant condition behaves very
                // similarly to a Wasm `block`. Therefore we can apply the same
                // optimization and inherit the [`Instruction::ConsumeFuel`] of
                // the parent control frame.
                let fuel_instr = self.fuel_instr();
                (reachability, fuel_instr)
            }
            TypedProvider::Register(condition) => {
                // Push the `if` parameters on the `else` provider stack for
                // later use in case we eventually visit the `else` branch.
                self.stack
                    .peek_n(usize::from(len_block_params), &mut self.providers);
                self.control_stack
                    .push_else_providers(self.providers.iter().copied())?;
                // Note: We increase preservation register usage of else providers
                //       so that they cannot be invalidated in the `then` block before
                //       arriving at the `else` block of an `if`.
                //       We manually decrease the usage of the else providers at the
                //       end of the `else`, or `if` in case `else` is missing.
                self.providers
                    .iter()
                    .copied()
                    .filter_map(TypedProvider::into_register)
                    .for_each(|register| self.stack.inc_register_usage(register));
                // Create the `else` label and the conditional branch to `else`.
                let else_label = self.instr_encoder.new_label();
                self.instr_encoder
                    .encode_branch_eqz(&mut self.stack, condition, else_label)?;
                let reachability = IfReachability::both(else_label);
                // Optionally create the [`Instruction::ConsumeFuel`] for the `then` branch.
                //
                // # Note
                //
                // The [`Instruction::ConsumeFuel`] for the `else` branch is
                // created on the fly when visiting the `else` block.
                let fuel_instr = self.make_fuel_instr()?;
                (reachability, fuel_instr)
            }
        };
        self.control_stack.push_frame(IfControlFrame::new(
            block_type,
            end_label,
            branch_params,
            stack_height,
            fuel_instr,
            reachability,
        ));
        Ok(())
    }

    fn visit_else(&mut self) -> Self::Output {
        let mut frame = match self.control_stack.pop_frame() {
            ControlFrame::If(frame) => frame,
            ControlFrame::Unreachable(frame @ UnreachableControlFrame::If) => {
                // Case: `else` branch for unreachable `if` block.
                //
                // In this case we can simply ignore the entire `else`
                // branch since it is unreachable anyways.
                self.control_stack.push_frame(frame);
                return Ok(());
            }
            unexpected => panic!(
                "expected `if` control flow frame on top for `else` but found: {:?}",
                unexpected,
            ),
        };
        frame.visited_else();
        if frame.is_then_reachable() {
            frame.update_end_of_then_reachability(self.reachable);
        }
        if let Some(else_label) = frame.else_label() {
            // Case: the `if` control frame has reachable `then` and `else` branches.
            debug_assert!(frame.is_then_reachable());
            debug_assert!(frame.is_else_reachable());
            if self.reachable {
                self.translate_copy_branch_params(&frame)?;
                let end_offset = self.instr_encoder.try_resolve_label(frame.end_label())?;
                // We are jumping to the end of the `if` so technically we need to bump branches.
                frame.branch_to();
                self.push_base_instr(Instruction::branch(end_offset))?;
            }
            self.instr_encoder.pin_label(else_label);
            if let Some(fuel_instr) = self.make_fuel_instr()? {
                frame.update_consume_fuel_instr(fuel_instr);
            }
            // At this point we can restore the `else` branch parameters
            // so that the `else` branch translation has the same set of
            // parameters as the `then` branch.
            self.stack.trunc(frame.block_height().into_u16() as usize);
            for provider in self.control_stack.pop_else_providers() {
                self.stack.push_provider(provider)?;
                if let TypedProvider::Register(register) = provider {
                    self.stack.dec_register_usage(register);
                }
            }
        }
        self.reachable = frame.is_else_reachable();
        // At last we need to push the popped and adjusted [`IfControlFrame`] back.
        self.control_stack.push_frame(frame);
        Ok(())
    }

    fn visit_end(&mut self) -> Self::Output {
        match self.control_stack.pop_frame() {
            ControlFrame::Block(frame) => self.translate_end_block(frame),
            ControlFrame::Loop(frame) => self.translate_end_loop(frame),
            ControlFrame::If(frame) => self.translate_end_if(frame),
            ControlFrame::Unreachable(frame) => self.translate_end_unreachable(frame),
        }?;
        self.instr_encoder.reset_last_instr();
        Ok(())
    }

    fn visit_br(&mut self, relative_depth: u32) -> Self::Output {
        bail_unreachable!(self);
        self.translate_br(relative_depth)
    }

    fn visit_br_if(&mut self, relative_depth: u32) -> Self::Output {
        bail_unreachable!(self);
        let engine = self.engine().clone();
        let condition = match self.stack.pop() {
            Provider::Const(condition) => {
                if i32::from(condition) != 0 {
                    // Case: `condition != 0` so the branch is always taken.
                    // Therefore we can simplify the `br_if` to a `br` instruction.
                    self.translate_br(relative_depth)?;
                }
                return Ok(());
            }
            Provider::Register(condition) => condition,
        };
        let fuel_info = self.fuel_info();
        let frame = match self.control_stack.acquire_target(relative_depth) {
            AcquiredTarget::Return(frame) => frame,
            AcquiredTarget::Branch(frame) => frame,
        };
        frame.branch_to();
        let branch_dst = frame.branch_destination();
        let branch_params = frame.branch_params(&engine);
        if branch_params.is_empty() {
            // Case: no values need to be copied so we can directly
            //       encode the `br_if` as efficient `branch_nez`.
            self.instr_encoder
                .encode_branch_nez(&mut self.stack, condition, branch_dst)?;
            return Ok(());
        }
        self.stack
            .peek_n(usize::from(branch_params.len()), &mut self.providers);
        if self
            .providers
            .iter()
            .copied()
            .eq(branch_params.iter().map(TypedProvider::Register))
        {
            // Case: the providers on the stack are already as
            //       expected by the branch params and therefore
            //       no copies are required.
            //
            // This means we can encode the `br_if` as efficient `branch_nez`.
            self.instr_encoder
                .encode_branch_nez(&mut self.stack, condition, branch_dst)?;
            return Ok(());
        }
        // Case: We need to copy the branch inputs to where the
        //       control frame expects them before actually branching
        //       to it.
        //       We do this by performing a negated `br_eqz` and skip
        //       the copy process with it in cases where no branch is
        //       needed.
        //       Otherwise we copy the values to their expected locations
        //       and finally perform the actual branch to the target
        //       control frame.
        let skip_label = self.instr_encoder.new_label();
        self.instr_encoder
            .encode_branch_eqz(&mut self.stack, condition, skip_label)?;
        self.instr_encoder.encode_copies(
            &mut self.stack,
            branch_params,
            &self.providers[..],
            &fuel_info,
        )?;
        let branch_offset = self.instr_encoder.try_resolve_label(branch_dst)?;
        self.push_base_instr(Instruction::branch(branch_offset))?;
        self.instr_encoder.pin_label(skip_label);
        Ok(())
    }

    fn visit_br_table(&mut self, targets: wasmparser::BrTable<'a>) -> Self::Output {
        bail_unreachable!(self);
        self.translate_br_table(targets)
    }

    fn visit_return(&mut self) -> Self::Output {
        bail_unreachable!(self);
        self.translate_return()
    }

    fn visit_call(&mut self, function_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let func_idx = FuncIdx::from(function_index);
        let func_type = self.func_type_of(func_idx);
        let (params, results) = func_type.params_results();
        self.stack.pop_n(params.len(), &mut self.providers);
        let results = self.stack.push_dynamic_n(results.len())?;
        let instr = match self.module.get_engine_func(func_idx) {
            Some(engine_func) => {
                // Case: We are calling an internal function and can optimize
                //       this case by using the special instruction for it.
                match params.len() {
                    0 => Instruction::call_internal_0(results, engine_func),
                    _ => Instruction::call_internal(results, engine_func),
                }
            }
            None => {
                // Case: We are calling an imported function and must use the
                //       general calling operator for it.
                match params.len() {
                    0 => Instruction::call_imported_0(results, function_index),
                    _ => Instruction::call_imported(results, function_index),
                }
            }
        };
        self.push_fueled_instr(instr, FuelCostsProvider::call)?;
        self.instr_encoder
            .encode_register_list(&mut self.stack, &self.providers)?;
        Ok(())
    }

    fn visit_call_indirect(&mut self, type_index: u32, table_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let type_index = FuncType::from(type_index);
        let func_type = self.func_type_at(type_index);
        let index = self.stack.pop();
        let indirect_params = self.call_indirect_params(index, table_index)?;
        let (params, results) = func_type.params_results();
        self.stack.pop_n(params.len(), &mut self.providers);
        let results = self.stack.push_dynamic_n(results.len())?;
        let instr = match (params.len(), indirect_params) {
            (0, Instruction::CallIndirectParams { .. }) => {
                Instruction::call_indirect_0(results, type_index)
            }
            (0, Instruction::CallIndirectParamsImm16 { .. }) => {
                Instruction::call_indirect_0_imm16(results, type_index)
            }
            (_, Instruction::CallIndirectParams { .. }) => {
                Instruction::call_indirect(results, type_index)
            }
            (_, Instruction::CallIndirectParamsImm16 { .. }) => {
                Instruction::call_indirect_imm16(results, type_index)
            }
            _ => unreachable!(),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::call)?;
        self.instr_encoder.append_instr(indirect_params)?;
        self.instr_encoder
            .encode_register_list(&mut self.stack, &self.providers)?;
        Ok(())
    }

    fn visit_return_call(&mut self, function_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let func_idx = FuncIdx::from(function_index);
        let func_type = self.func_type_of(func_idx);
        let params = func_type.params();
        self.stack.pop_n(params.len(), &mut self.providers);
        let instr = match self.module.get_engine_func(func_idx) {
            Some(engine_func) => {
                // Case: We are calling an internal function and can optimize
                //       this case by using the special instruction for it.
                match params.len() {
                    0 => Instruction::return_call_internal_0(engine_func),
                    _ => Instruction::return_call_internal(engine_func),
                }
            }
            None => {
                // Case: We are calling an imported function and must use the
                //       general calling operator for it.
                match params.len() {
                    0 => Instruction::return_call_imported_0(function_index),
                    _ => Instruction::return_call_imported(function_index),
                }
            }
        };
        self.push_fueled_instr(instr, FuelCostsProvider::call)?;
        self.instr_encoder
            .encode_register_list(&mut self.stack, &self.providers)?;
        self.reachable = false;
        Ok(())
    }

    fn visit_return_call_indirect(&mut self, type_index: u32, table_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let type_index = FuncType::from(type_index);
        let func_type = self.func_type_at(type_index);
        let params = func_type.params();
        let index = self.stack.pop();
        let indirect_params = self.call_indirect_params(index, table_index)?;
        self.stack.pop_n(params.len(), &mut self.providers);
        let instr = match (params.len(), indirect_params) {
            (0, Instruction::CallIndirectParams { .. }) => {
                Instruction::return_call_indirect_0(type_index)
            }
            (0, Instruction::CallIndirectParamsImm16 { .. }) => {
                Instruction::return_call_indirect_0_imm16(type_index)
            }
            (_, Instruction::CallIndirectParams { .. }) => {
                Instruction::return_call_indirect(type_index)
            }
            (_, Instruction::CallIndirectParamsImm16 { .. }) => {
                Instruction::return_call_indirect_imm16(type_index)
            }
            _ => unreachable!(),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::call)?;
        self.instr_encoder.append_instr(indirect_params)?;
        self.instr_encoder
            .encode_register_list(&mut self.stack, &self.providers)?;
        self.reachable = false;
        Ok(())
    }

    fn visit_drop(&mut self) -> Self::Output {
        bail_unreachable!(self);
        self.stack.drop();
        Ok(())
    }

    fn visit_select(&mut self) -> Self::Output {
        self.translate_select(None)
    }

    fn visit_typed_select(&mut self, ty: wasmparser::ValType) -> Self::Output {
        let type_hint = WasmiValueType::from(ty).into_inner();
        self.translate_select(Some(type_hint))
    }

    fn visit_local_get(&mut self, local_index: u32) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_local(local_index)?;
        Ok(())
    }

    fn visit_local_set(&mut self, local_index: u32) -> Self::Output {
        bail_unreachable!(self);
        self.stack.gc_preservations();
        let value = self.stack.pop();
        let local = Reg::try_from(local_index)?;
        if let TypedProvider::Register(value) = value {
            if value == local {
                // Case: `(local.set $n (local.get $n))` is a no-op so we can ignore it.
                //
                // Note: This does not require any preservation since it won't change
                //       the value of `local $n`.
                return Ok(());
            }
        }
        let preserved = self.stack.preserve_locals(local_index)?;
        let fuel_info = self.fuel_info();
        self.instr_encoder.encode_local_set(
            &mut self.stack,
            &self.module,
            local,
            value,
            preserved,
            fuel_info,
        )?;
        Ok(())
    }

    fn visit_local_tee(&mut self, local_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let input = self.stack.peek();
        self.visit_local_set(local_index)?;
        match input {
            Provider::Register(_register) => {
                self.stack.push_local(local_index)?;
            }
            Provider::Const(value) => {
                self.stack.push_const(value);
            }
        }
        Ok(())
    }

    fn visit_global_get(&mut self, global_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let global_idx = module::GlobalIdx::from(global_index);
        let (global_type, init_value) = self.module.get_global(global_idx);
        let content = global_type.content();
        if let (Mutability::Const, Some(init_expr)) = (global_type.mutability(), init_value) {
            if let Some(value) = init_expr.eval_const() {
                // Optimization: Access to immutable internally defined global variables
                //               can be replaced with their constant initialization value.
                self.stack.push_const(TypedVal::new(content, value));
                return Ok(());
            }
            if let Some(func_index) = init_expr.funcref() {
                // Optimization: Forward to `ref.func x` translation.
                self.visit_ref_func(func_index.into_u32())?;
                return Ok(());
            }
        }
        // Case: The `global.get` instruction accesses a mutable or imported
        //       global variable and thus cannot be optimized away.
        let global_idx = ir::index::Global::from(global_index);
        let result = self.stack.push_dynamic()?;
        self.push_fueled_instr(
            Instruction::global_get(result, global_idx),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_global_set(&mut self, global_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let global = ir::index::Global::from(global_index);
        match self.stack.pop() {
            TypedProvider::Register(input) => {
                self.push_fueled_instr(
                    Instruction::global_set(input, global),
                    FuelCostsProvider::instance,
                )?;
                Ok(())
            }
            TypedProvider::Const(input) => {
                let (global_type, _init_value) = self
                    .module
                    .get_global(module::GlobalIdx::from(global_index));
                debug_assert_eq!(global_type.content(), input.ty());
                match global_type.content() {
                    ValType::I32 => {
                        if let Ok(value) = Const16::try_from(i32::from(input)) {
                            self.push_fueled_instr(
                                Instruction::global_set_i32imm16(value, global),
                                FuelCostsProvider::instance,
                            )?;
                            return Ok(());
                        }
                    }
                    ValType::I64 => {
                        if let Ok(value) = Const16::try_from(i64::from(input)) {
                            self.push_fueled_instr(
                                Instruction::global_set_i64imm16(value, global),
                                FuelCostsProvider::instance,
                            )?;
                            return Ok(());
                        }
                    }
                    _ => {}
                };
                let cref = self.stack.alloc_const(input)?;
                self.push_fueled_instr(
                    Instruction::global_set(cref, global),
                    FuelCostsProvider::instance,
                )?;
                Ok(())
            }
        }
    }

    fn visit_i32_load(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::load32,
            Instruction::load32_offset16,
            Instruction::load32_at,
        )
    }

    fn visit_i64_load(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::load64,
            Instruction::load64_offset16,
            Instruction::load64_at,
        )
    }

    fn visit_f32_load(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::load32,
            Instruction::load32_offset16,
            Instruction::load32_at,
        )
    }

    fn visit_f64_load(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::load64,
            Instruction::load64_offset16,
            Instruction::load64_at,
        )
    }

    fn visit_i32_load8_s(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i32_load8_s,
            Instruction::i32_load8_s_offset16,
            Instruction::i32_load8_s_at,
        )
    }

    fn visit_i32_load8_u(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i32_load8_u,
            Instruction::i32_load8_u_offset16,
            Instruction::i32_load8_u_at,
        )
    }

    fn visit_i32_load16_s(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i32_load16_s,
            Instruction::i32_load16_s_offset16,
            Instruction::i32_load16_s_at,
        )
    }

    fn visit_i32_load16_u(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i32_load16_u,
            Instruction::i32_load16_u_offset16,
            Instruction::i32_load16_u_at,
        )
    }

    fn visit_i64_load8_s(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load8_s,
            Instruction::i64_load8_s_offset16,
            Instruction::i64_load8_s_at,
        )
    }

    fn visit_i64_load8_u(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load8_u,
            Instruction::i64_load8_u_offset16,
            Instruction::i64_load8_u_at,
        )
    }

    fn visit_i64_load16_s(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load16_s,
            Instruction::i64_load16_s_offset16,
            Instruction::i64_load16_s_at,
        )
    }

    fn visit_i64_load16_u(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load16_u,
            Instruction::i64_load16_u_offset16,
            Instruction::i64_load16_u_at,
        )
    }

    fn visit_i64_load32_s(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load32_s,
            Instruction::i64_load32_s_offset16,
            Instruction::i64_load32_s_at,
        )
    }

    fn visit_i64_load32_u(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_load(
            memarg,
            Instruction::i64_load32_u,
            Instruction::i64_load32_u_offset16,
            Instruction::i64_load32_u_at,
        )
    }

    fn visit_i32_store(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore::<i32, i16>(
            memarg,
            Instruction::store32,
            Instruction::i32_store_imm16,
            Instruction::store32_offset16,
            Instruction::i32_store_offset16_imm16,
            Instruction::store32_at,
            Instruction::i32_store_at_imm16,
        )
    }

    fn visit_i64_store(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore::<i64, i16>(
            memarg,
            Instruction::store64,
            Instruction::i64_store_imm16,
            Instruction::store64_offset16,
            Instruction::i64_store_offset16_imm16,
            Instruction::store64_at,
            Instruction::i64_store_at_imm16,
        )
    }

    fn visit_f32_store(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_store(
            memarg,
            Instruction::store32,
            Instruction::store32_offset16,
            Instruction::store32_at,
        )
    }

    fn visit_f64_store(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_store(
            memarg,
            Instruction::store64,
            Instruction::store64_offset16,
            Instruction::store64_at,
        )
    }

    fn visit_i32_store8(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore_wrap::<i32, i8, i8>(
            memarg,
            Instruction::i32_store8,
            Instruction::i32_store8_imm,
            Instruction::i32_store8_offset16,
            Instruction::i32_store8_offset16_imm,
            Instruction::i32_store8_at,
            Instruction::i32_store8_at_imm,
        )
    }

    fn visit_i32_store16(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore_wrap::<i32, i16, i16>(
            memarg,
            Instruction::i32_store16,
            Instruction::i32_store16_imm,
            Instruction::i32_store16_offset16,
            Instruction::i32_store16_offset16_imm,
            Instruction::i32_store16_at,
            Instruction::i32_store16_at_imm,
        )
    }

    fn visit_i64_store8(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore_wrap::<i64, i8, i8>(
            memarg,
            Instruction::i64_store8,
            Instruction::i64_store8_imm,
            Instruction::i64_store8_offset16,
            Instruction::i64_store8_offset16_imm,
            Instruction::i64_store8_at,
            Instruction::i64_store8_at_imm,
        )
    }

    fn visit_i64_store16(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore_wrap::<i64, i16, i16>(
            memarg,
            Instruction::i64_store16,
            Instruction::i64_store16_imm,
            Instruction::i64_store16_offset16,
            Instruction::i64_store16_offset16_imm,
            Instruction::i64_store16_at,
            Instruction::i64_store16_at_imm,
        )
    }

    fn visit_i64_store32(&mut self, memarg: wasmparser::MemArg) -> Self::Output {
        self.translate_istore_wrap::<i64, i32, i16>(
            memarg,
            Instruction::i64_store32,
            Instruction::i64_store32_imm16,
            Instruction::i64_store32_offset16,
            Instruction::i64_store32_offset16_imm16,
            Instruction::i64_store32_at,
            Instruction::i64_store32_at_imm16,
        )
    }

    fn visit_memory_size(&mut self, mem: u32) -> Self::Output {
        bail_unreachable!(self);
        let memory = index::Memory::from(mem);
        let result = self.stack.push_dynamic()?;
        self.push_fueled_instr(
            Instruction::memory_size(result, memory),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_memory_grow(&mut self, mem: u32) -> Self::Output {
        bail_unreachable!(self);
        let memory = index::Memory::from(mem);
        let memory_type = *self.module.get_type_of_memory(MemoryIdx::from(mem));
        let delta = self.stack.pop();
        let delta = self.as_index_type_const32(delta, memory_type.index_ty())?;
        let result = self.stack.push_dynamic()?;
        if let Provider::Const(delta) = delta {
            if u64::from(delta) == 0 {
                // Case: growing by 0 pages.
                //
                // Since `memory.grow` returns the `memory.size` before the
                // operation a `memory.grow` with `delta` of 0 can be translated
                // as `memory.size` instruction instead.
                self.push_fueled_instr(
                    Instruction::memory_size(result, memory),
                    FuelCostsProvider::instance,
                )?;
                return Ok(());
            }
        }
        let instr = match delta {
            Provider::Const(delta) => Instruction::memory_grow_imm(result, delta),
            Provider::Register(delta) => Instruction::memory_grow(result, delta),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::memory_index(memory))?;
        Ok(())
    }

    fn visit_i32_const(&mut self, value: i32) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_const(value);
        Ok(())
    }

    fn visit_i64_const(&mut self, value: i64) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_const(value);
        Ok(())
    }

    fn visit_f32_const(&mut self, value: wasmparser::Ieee32) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_const(F32::from_bits(value.bits()));
        Ok(())
    }

    fn visit_f64_const(&mut self, value: wasmparser::Ieee64) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_const(F64::from_bits(value.bits()));
        Ok(())
    }

    fn visit_ref_null(&mut self, hty: wasmparser::HeapType) -> Self::Output {
        bail_unreachable!(self);
        let type_hint = WasmiValueType::from(hty).into_inner();
        let null = match type_hint {
            ValType::FuncRef => TypedVal::from(FuncRef::null()),
            ValType::ExternRef => TypedVal::from(ExternRef::null()),
            _ => panic!("must be a Wasm reftype"),
        };
        self.stack.push_const(null);
        Ok(())
    }

    fn visit_ref_is_null(&mut self) -> Self::Output {
        bail_unreachable!(self);
        let input = self.stack.peek();
        if let Provider::Const(input) = input {
            self.stack.drop();
            let untyped = input.untyped();
            let is_null = match input.ty() {
                ValType::FuncRef => FuncRef::from(untyped).is_null(),
                ValType::ExternRef => ExternRef::from(untyped).is_null(),
                invalid => panic!("ref.is_null: encountered invalid input type: {invalid:?}"),
            };
            self.stack.push_const(i32::from(is_null));
            return Ok(());
        }
        // Note: Since `funcref` and `externref` both serialize to `UntypedValue`
        //       as raw `u64` values we can use `i64.eqz` translation for `ref.is_null`.
        self.visit_i64_eqz()
    }

    fn visit_ref_func(&mut self, function_index: u32) -> Self::Output {
        bail_unreachable!(self);
        let result = self.stack.push_dynamic()?;
        self.push_fueled_instr(
            Instruction::ref_func(result, function_index),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_i32_eqz(&mut self) -> Self::Output {
        bail_unreachable!(self);
        self.stack.push_const(0_i32);
        self.visit_i32_eq()
    }

    fn visit_i32_eq(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, bool>(
            Instruction::i32_eq,
            Instruction::i32_eq_imm16,
            wasm::i32_eq,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x == x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i32| {
                this.instr_encoder
                    .fuse_eqz::<i32>(&mut this.stack, lhs, rhs)
            },
        )
    }

    fn visit_i32_ne(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, bool>(
            Instruction::i32_ne,
            Instruction::i32_ne_imm16,
            wasm::i32_ne,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x != x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i32| {
                this.instr_encoder
                    .fuse_nez::<i32>(&mut this.stack, lhs, rhs)
            },
        )
    }

    fn visit_i32_lt_s(&mut self) -> Self::Output {
        self.translate_binary::<i32, bool>(
            Instruction::i32_lt_s,
            Instruction::i32_lt_s_imm16_rhs,
            Instruction::i32_lt_s_imm16_lhs,
            wasm::i32_lt_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i32| {
                if rhs == i32::MIN {
                    // Optimization: `x < MIN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i32, _rhs: Reg| {
                if lhs == i32::MAX {
                    // Optimization: `MAX < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_lt_u(&mut self) -> Self::Output {
        self.translate_binary::<u32, bool>(
            Instruction::i32_lt_u,
            Instruction::i32_lt_u_imm16_rhs,
            Instruction::i32_lt_u_imm16_lhs,
            wasm::i32_lt_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u32| {
                if rhs == u32::MIN {
                    // Optimization: `x < MIN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u32, _rhs: Reg| {
                if lhs == u32::MAX {
                    // Optimization: `MAX < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_gt_s(&mut self) -> Self::Output {
        self.translate_binary::<i32, bool>(
            swap_ops!(Instruction::i32_lt_s),
            swap_ops!(Instruction::i32_lt_s_imm16_lhs),
            swap_ops!(Instruction::i32_lt_s_imm16_rhs),
            wasm::i32_gt_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i32| {
                if rhs == i32::MAX {
                    // Optimization: `x > MAX` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i32, _rhs: Reg| {
                if lhs == i32::MIN {
                    // Optimization: `MIN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_gt_u(&mut self) -> Self::Output {
        self.translate_binary::<u32, bool>(
            swap_ops!(Instruction::i32_lt_u),
            swap_ops!(Instruction::i32_lt_u_imm16_lhs),
            swap_ops!(Instruction::i32_lt_u_imm16_rhs),
            wasm::i32_gt_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u32| {
                if rhs == u32::MAX {
                    // Optimization: `x > MAX` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u32, _rhs: Reg| {
                if lhs == u32::MIN {
                    // Optimization: `MIN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_le_s(&mut self) -> Self::Output {
        self.translate_binary::<i32, bool>(
            Instruction::i32_le_s,
            Instruction::i32_le_s_imm16_rhs,
            Instruction::i32_le_s_imm16_lhs,
            wasm::i32_le_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i32| {
                if rhs == i32::MAX {
                    // Optimization: `x <= MAX` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i32, _rhs: Reg| {
                if lhs == i32::MIN {
                    // Optimization: `MIN <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_le_u(&mut self) -> Self::Output {
        self.translate_binary::<u32, bool>(
            Instruction::i32_le_u,
            Instruction::i32_le_u_imm16_rhs,
            Instruction::i32_le_u_imm16_lhs,
            wasm::i32_le_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u32| {
                if rhs == u32::MAX {
                    // Optimization: `x <= MAX` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u32, _rhs: Reg| {
                if lhs == u32::MIN {
                    // Optimization: `MIN <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_ge_s(&mut self) -> Self::Output {
        self.translate_binary::<i32, bool>(
            swap_ops!(Instruction::i32_le_s),
            swap_ops!(Instruction::i32_le_s_imm16_lhs),
            swap_ops!(Instruction::i32_le_s_imm16_rhs),
            wasm::i32_ge_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i32| {
                if rhs == i32::MIN {
                    // Optimization: `x >= MIN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i32, _rhs: Reg| {
                if lhs == i32::MAX {
                    // Optimization: `MAX >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_ge_u(&mut self) -> Self::Output {
        self.translate_binary::<u32, bool>(
            swap_ops!(Instruction::i32_le_u),
            swap_ops!(Instruction::i32_le_u_imm16_lhs),
            swap_ops!(Instruction::i32_le_u_imm16_rhs),
            wasm::i32_ge_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u32| {
                if rhs == u32::MIN {
                    // Optimization: `x >= MIN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u32, _rhs: Reg| {
                if lhs == u32::MAX {
                    // Optimization: `MAX >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_eqz(&mut self) -> Self::Output {
        bail_unreachable!(self);
        // Push a zero on the value stack so we can translate `i64.eqz` as `i64.eq(x, 0)`.
        self.stack.push_const(0_i64);
        self.visit_i64_eq()
    }

    fn visit_i64_eq(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, bool>(
            Instruction::i64_eq,
            Instruction::i64_eq_imm16,
            wasm::i64_eq,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x == x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i64| {
                this.instr_encoder
                    .fuse_eqz::<i64>(&mut this.stack, lhs, rhs)
            },
        )
    }

    fn visit_i64_ne(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, bool>(
            Instruction::i64_ne,
            Instruction::i64_ne_imm16,
            wasm::i64_ne,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x != x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i64| {
                this.instr_encoder
                    .fuse_nez::<i64>(&mut this.stack, lhs, rhs)
            },
        )
    }

    fn visit_i64_lt_s(&mut self) -> Self::Output {
        self.translate_binary::<i64, bool>(
            Instruction::i64_lt_s,
            Instruction::i64_lt_s_imm16_rhs,
            Instruction::i64_lt_s_imm16_lhs,
            wasm::i64_lt_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i64| {
                if rhs == i64::MIN {
                    // Optimization: `x < MIN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i64, _rhs: Reg| {
                if lhs == i64::MAX {
                    // Optimization: `MAX < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_lt_u(&mut self) -> Self::Output {
        self.translate_binary::<u64, bool>(
            Instruction::i64_lt_u,
            Instruction::i64_lt_u_imm16_rhs,
            Instruction::i64_lt_u_imm16_lhs,
            wasm::i64_lt_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u64| {
                if rhs == u64::MIN {
                    // Optimization: `x < MIN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u64, _rhs: Reg| {
                if lhs == u64::MAX {
                    // Optimization: `MAX < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_gt_s(&mut self) -> Self::Output {
        self.translate_binary::<i64, bool>(
            swap_ops!(Instruction::i64_lt_s),
            swap_ops!(Instruction::i64_lt_s_imm16_lhs),
            swap_ops!(Instruction::i64_lt_s_imm16_rhs),
            wasm::i64_gt_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i64| {
                if rhs == i64::MAX {
                    // Optimization: `x > MAX` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i64, _rhs: Reg| {
                if lhs == i64::MIN {
                    // Optimization: `MIN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_gt_u(&mut self) -> Self::Output {
        self.translate_binary::<u64, bool>(
            swap_ops!(Instruction::i64_lt_u),
            swap_ops!(Instruction::i64_lt_u_imm16_lhs),
            swap_ops!(Instruction::i64_lt_u_imm16_rhs),
            wasm::i64_gt_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u64| {
                if rhs == u64::MAX {
                    // Optimization: `x > MAX` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u64, _rhs: Reg| {
                if lhs == u64::MIN {
                    // Optimization: `MIN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_le_s(&mut self) -> Self::Output {
        self.translate_binary::<i64, bool>(
            Instruction::i64_le_s,
            Instruction::i64_le_s_imm16_rhs,
            Instruction::i64_le_s_imm16_lhs,
            wasm::i64_le_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i64| {
                if rhs == i64::MAX {
                    // Optimization: `x <= MAX` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i64, _rhs: Reg| {
                if lhs == i64::MIN {
                    // Optimization: `MIN <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_le_u(&mut self) -> Self::Output {
        self.translate_binary::<u64, bool>(
            Instruction::i64_le_u,
            Instruction::i64_le_u_imm16_rhs,
            Instruction::i64_le_u_imm16_lhs,
            wasm::i64_le_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u64| {
                if rhs == u64::MAX {
                    // Optimization: `x <= MAX` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u64, _rhs: Reg| {
                if lhs == u64::MIN {
                    // Optimization: `MIN <= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_ge_s(&mut self) -> Self::Output {
        self.translate_binary::<i64, bool>(
            swap_ops!(Instruction::i64_le_s),
            swap_ops!(Instruction::i64_le_s_imm16_lhs),
            swap_ops!(Instruction::i64_le_s_imm16_rhs),
            wasm::i64_ge_s,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: i64| {
                if rhs == i64::MIN {
                    // Optimization: `x >= MIN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: i64, _rhs: Reg| {
                if lhs == i64::MAX {
                    // Optimization: `MAX >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_ge_u(&mut self) -> Self::Output {
        self.translate_binary::<u64, bool>(
            swap_ops!(Instruction::i64_le_u),
            swap_ops!(Instruction::i64_le_u_imm16_lhs),
            swap_ops!(Instruction::i64_le_u_imm16_rhs),
            wasm::i64_ge_u,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: u64| {
                if rhs == u64::MIN {
                    // Optimization: `x >= MIN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: u64, _rhs: Reg| {
                if lhs == u64::MAX {
                    // Optimization: `MAX >= x` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_eq(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, bool>(
            Instruction::f32_eq,
            wasm::f32_eq,
            Self::no_custom_opt,
            |this, _reg_in: Reg, imm_in: f32| {
                if imm_in.is_nan() {
                    // Optimization: `NaN == x` or `x == NaN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_ne(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, bool>(
            Instruction::f32_ne,
            wasm::f32_ne,
            Self::no_custom_opt,
            |this, _reg_in: Reg, imm_in: f32| {
                if imm_in.is_nan() {
                    // Optimization: `NaN != x` or `x != NaN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_lt(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, bool>(
            Instruction::f32_lt,
            wasm::f32_lt,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: f32| {
                if rhs.is_nan() {
                    // Optimization: `x < NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if rhs.is_infinite() && rhs.is_sign_negative() {
                    // Optimization: `x < -INF` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f32, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if lhs.is_infinite() && lhs.is_sign_positive() {
                    // Optimization: `+INF < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_gt(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, bool>(
            swap_ops!(Instruction::f32_lt),
            wasm::f32_gt,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: f32| {
                if rhs.is_nan() {
                    // Optimization: `x > NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if rhs.is_infinite() && rhs.is_sign_positive() {
                    // Optimization: `x > INF` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f32, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if lhs.is_infinite() && lhs.is_sign_negative() {
                    // Optimization: `-INF > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_le(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, bool>(
            Instruction::f32_le,
            wasm::f32_le,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: f32| {
                if rhs.is_nan() {
                    // Optimization: `x <= NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f32, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN <= x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_ge(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, bool>(
            swap_ops!(Instruction::f32_le),
            wasm::f32_ge,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: f32| {
                if rhs.is_nan() {
                    // Optimization: `x >= NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f32, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN >= x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_eq(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, bool>(
            Instruction::f64_eq,
            wasm::f64_eq,
            Self::no_custom_opt,
            |this, _reg_in: Reg, imm_in: f64| {
                if imm_in.is_nan() {
                    // Optimization: `NaN == x` or `x == NaN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_ne(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, bool>(
            Instruction::f64_ne,
            wasm::f64_ne,
            Self::no_custom_opt,
            |this, _reg_in: Reg, imm_in: f64| {
                if imm_in.is_nan() {
                    // Optimization: `NaN != x` or `x != NaN` is always `true`
                    this.stack.push_const(true);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_lt(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, bool>(
            Instruction::f64_lt,
            wasm::f64_lt,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: f64| {
                if rhs.is_nan() {
                    // Optimization: `x < NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if rhs.is_infinite() && rhs.is_sign_negative() {
                    // Optimization: `x < -INF` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f64, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if lhs.is_infinite() && lhs.is_sign_positive() {
                    // Optimization: `+INF < x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_gt(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, bool>(
            swap_ops!(Instruction::f64_lt),
            wasm::f64_gt,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `x > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, _lhs: Reg, rhs: f64| {
                if rhs.is_nan() {
                    // Optimization: `x > NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if rhs.is_infinite() && rhs.is_sign_positive() {
                    // Optimization: `x > INF` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f64, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                if lhs.is_infinite() && lhs.is_sign_negative() {
                    // Optimization: `-INF > x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_le(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, bool>(
            Instruction::f64_le,
            wasm::f64_le,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: f64| {
                if rhs.is_nan() {
                    // Optimization: `x <= NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f64, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN <= x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_ge(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, bool>(
            swap_ops!(Instruction::f64_le),
            wasm::f64_ge,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: f64| {
                if rhs.is_nan() {
                    // Optimization: `x >= NAN` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: f64, _rhs: Reg| {
                if lhs.is_nan() {
                    // Optimization: `NAN >= x` is always `false`
                    this.stack.push_const(false);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_clz(&mut self) -> Self::Output {
        self.translate_unary::<i32, i32>(Instruction::i32_clz, wasm::i32_clz)
    }

    fn visit_i32_ctz(&mut self) -> Self::Output {
        self.translate_unary::<i32, i32>(Instruction::i32_ctz, wasm::i32_ctz)
    }

    fn visit_i32_popcnt(&mut self) -> Self::Output {
        self.translate_unary::<i32, i32>(Instruction::i32_popcnt, wasm::i32_popcnt)
    }

    fn visit_i32_add(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, i32>(
            Instruction::i32_add,
            Instruction::i32_add_imm16,
            wasm::i32_add,
            Self::no_custom_opt,
            |this, reg: Reg, value: i32| {
                if value == 0 {
                    // Optimization: `add x + 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_sub(&mut self) -> Self::Output {
        self.translate_binary::<i32, i32>(
            Instruction::i32_sub,
            |_, _, _| unreachable!("`i32.sub r c` is translated as `i32.add r -c`"),
            Instruction::i32_sub_imm16_lhs,
            wasm::i32_sub,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `sub x - x` is always `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i32| {
                if rhs == 0 {
                    // Optimization: `sub x - 0` is same as `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                if this.try_push_binary_instr_imm16(
                    lhs,
                    rhs.wrapping_neg(),
                    Instruction::i32_add_imm16,
                )? {
                    // Simplification: Translate `i32.sub r c` as `i32.add r -c`
                    return Ok(true);
                }
                this.push_binary_instr_imm(lhs, rhs.wrapping_neg(), Instruction::i32_add)?;
                Ok(true)
            },
            Self::no_custom_opt,
        )
    }

    fn visit_i32_mul(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, i32>(
            Instruction::i32_mul,
            Instruction::i32_mul_imm16,
            wasm::i32_mul,
            Self::no_custom_opt,
            |this, reg: Reg, value: i32| {
                if value == 0 {
                    // Optimization: `add x * 0` is always `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                if value == 1 {
                    // Optimization: `add x * 1` is always `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_div_s(&mut self) -> Self::Output {
        self.translate_divrem::<i32>(
            Instruction::i32_div_s,
            Instruction::i32_div_s_imm16_rhs,
            Instruction::i32_div_s_imm16_lhs,
            wasm::i32_div_s,
            Self::no_custom_opt,
            |this, lhs: Reg, rhs: i32| {
                if rhs == 1 {
                    // Optimization: `x / 1` is always `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_div_u(&mut self) -> Self::Output {
        self.translate_divrem::<u32>(
            Instruction::i32_div_u,
            Instruction::i32_div_u_imm16_rhs,
            Instruction::i32_div_u_imm16_lhs,
            wasm::i32_div_u,
            Self::no_custom_opt,
            |this, lhs: Reg, rhs: u32| {
                if rhs == 1 {
                    // Optimization: `x / 1` is always `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_rem_s(&mut self) -> Self::Output {
        self.translate_divrem::<i32>(
            Instruction::i32_rem_s,
            Instruction::i32_rem_s_imm16_rhs,
            Instruction::i32_rem_s_imm16_lhs,
            wasm::i32_rem_s,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: i32| {
                if rhs == 1 || rhs == -1 {
                    // Optimization: `x % 1` or `x % -1` is always `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_rem_u(&mut self) -> Self::Output {
        self.translate_divrem::<u32>(
            Instruction::i32_rem_u,
            Instruction::i32_rem_u_imm16_rhs,
            Instruction::i32_rem_u_imm16_lhs,
            wasm::i32_rem_u,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: u32| {
                if rhs == 1 {
                    // Optimization: `x % 1` is always `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_and(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, i32>(
            Instruction::i32_bitand,
            Instruction::i32_bitand_imm16,
            wasm::i32_bitand,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x & x` is always just `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i32| {
                if value == -1 {
                    // Optimization: `x & -1` is same as `x`
                    //
                    // Note: This is due to the fact that -1
                    // in twos-complements only contains 1 bits.
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                if value == 0 {
                    // Optimization: `x & 0` is same as `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_or(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, i32>(
            Instruction::i32_bitor,
            Instruction::i32_bitor_imm16,
            wasm::i32_bitor,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x | x` is always just `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i32| {
                if value == -1 {
                    // Optimization: `x | -1` is same as `-1`
                    //
                    // Note: This is due to the fact that -1
                    // in twos-complements only contains 1 bits.
                    this.stack.push_const(-1_i32);
                    return Ok(true);
                }
                if value == 0 {
                    // Optimization: `x | 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_xor(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i32, i32>(
            Instruction::i32_bitxor,
            Instruction::i32_bitxor_imm16,
            wasm::i32_bitxor,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x ^ x` is always `0`
                    this.stack.push_const(0_i32);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i32| {
                if value == 0 {
                    // Optimization: `x ^ 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_shl(&mut self) -> Self::Output {
        self.translate_shift::<i32>(
            Instruction::i32_shl,
            Instruction::i32_shl_by,
            Instruction::i32_shl_imm16,
            wasm::i32_shl,
            Self::no_custom_opt,
        )
    }

    fn visit_i32_shr_s(&mut self) -> Self::Output {
        self.translate_shift::<i32>(
            Instruction::i32_shr_s,
            Instruction::i32_shr_s_by,
            Instruction::i32_shr_s_imm16,
            wasm::i32_shr_s,
            |this, lhs: i32, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1 >> x` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_shr_u(&mut self) -> Self::Output {
        self.translate_shift::<i32>(
            Instruction::i32_shr_u,
            Instruction::i32_shr_u_by,
            Instruction::i32_shr_u_imm16,
            wasm::i32_shr_u,
            Self::no_custom_opt,
        )
    }

    fn visit_i32_rotl(&mut self) -> Self::Output {
        self.translate_shift::<i32>(
            Instruction::i32_rotl,
            Instruction::i32_rotl_by,
            Instruction::i32_rotl_imm16,
            wasm::i32_rotl,
            |this, lhs: i32, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1.rotate_left(x)` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i32_rotr(&mut self) -> Self::Output {
        self.translate_shift::<i32>(
            Instruction::i32_rotr,
            Instruction::i32_rotr_by,
            Instruction::i32_rotr_imm16,
            wasm::i32_rotr,
            |this, lhs: i32, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1.rotate_right(x)` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_clz(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_clz, wasm::i64_clz)
    }

    fn visit_i64_ctz(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_ctz, wasm::i64_ctz)
    }

    fn visit_i64_popcnt(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_popcnt, wasm::i64_popcnt)
    }

    fn visit_i64_add(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, i64>(
            Instruction::i64_add,
            Instruction::i64_add_imm16,
            wasm::i64_add,
            Self::no_custom_opt,
            |this, reg: Reg, value: i64| {
                if value == 0 {
                    // Optimization: `add x + 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_sub(&mut self) -> Self::Output {
        self.translate_binary::<i64, i64>(
            Instruction::i64_sub,
            |_, _, _| unreachable!("`i64.sub r c` is translated as `i64.add r -c`"),
            Instruction::i64_sub_imm16_lhs,
            wasm::i64_sub,
            |this, lhs: Reg, rhs: Reg| {
                if lhs == rhs {
                    // Optimization: `sub x - x` is always `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, lhs: Reg, rhs: i64| {
                if rhs == 0 {
                    // Optimization: `sub x - 0` is same as `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                if this.try_push_binary_instr_imm16(
                    lhs,
                    rhs.wrapping_neg(),
                    Instruction::i64_add_imm16,
                )? {
                    // Simplification: Translate `i64.sub r c` as `i64.add r -c`
                    return Ok(true);
                }
                this.push_binary_instr_imm(lhs, rhs.wrapping_neg(), Instruction::i64_add)?;
                Ok(true)
            },
            Self::no_custom_opt,
        )
    }

    fn visit_i64_mul(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, i64>(
            Instruction::i64_mul,
            Instruction::i64_mul_imm16,
            wasm::i64_mul,
            Self::no_custom_opt,
            |this, reg: Reg, value: i64| {
                if value == 0 {
                    // Optimization: `add x * 0` is always `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                if value == 1 {
                    // Optimization: `add x * 1` is always `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_div_s(&mut self) -> Self::Output {
        self.translate_divrem::<i64>(
            Instruction::i64_div_s,
            Instruction::i64_div_s_imm16_rhs,
            Instruction::i64_div_s_imm16_lhs,
            wasm::i64_div_s,
            Self::no_custom_opt,
            |this, lhs: Reg, rhs: i64| {
                if rhs == 1 {
                    // Optimization: `x / 1` is always `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_div_u(&mut self) -> Self::Output {
        self.translate_divrem::<u64>(
            Instruction::i64_div_u,
            Instruction::i64_div_u_imm16_rhs,
            Instruction::i64_div_u_imm16_lhs,
            wasm::i64_div_u,
            Self::no_custom_opt,
            |this, lhs: Reg, rhs: u64| {
                if rhs == 1 {
                    // Optimization: `x / 1` is always `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_rem_s(&mut self) -> Self::Output {
        self.translate_divrem::<i64>(
            Instruction::i64_rem_s,
            Instruction::i64_rem_s_imm16_rhs,
            Instruction::i64_rem_s_imm16_lhs,
            wasm::i64_rem_s,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: i64| {
                if rhs == 1 || rhs == -1 {
                    // Optimization: `x % 1` or `x % -1` is always `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_rem_u(&mut self) -> Self::Output {
        self.translate_divrem::<u64>(
            Instruction::i64_rem_u,
            Instruction::i64_rem_u_imm16_rhs,
            Instruction::i64_rem_u_imm16_lhs,
            wasm::i64_rem_u,
            Self::no_custom_opt,
            |this, _lhs: Reg, rhs: u64| {
                if rhs == 1 {
                    // Optimization: `x % 1` is always `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_and(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, i64>(
            Instruction::i64_bitand,
            Instruction::i64_bitand_imm16,
            wasm::i64_bitand,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x & x` is always just `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i64| {
                if value == -1 {
                    // Optimization: `x & -1` is same as `x`
                    //
                    // Note: This is due to the fact that -1
                    // in twos-complements only contains 1 bits.
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                if value == 0 {
                    // Optimization: `x & 0` is same as `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_or(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, i64>(
            Instruction::i64_bitor,
            Instruction::i64_bitor_imm16,
            wasm::i64_bitor,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x | x` is always just `x`
                    this.stack.push_register(lhs)?;
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i64| {
                if value == -1 {
                    // Optimization: `x | -1` is same as `-1`
                    //
                    // Note: This is due to the fact that -1
                    // in twos-complements only contains 1 bits.
                    this.stack.push_const(-1_i64);
                    return Ok(true);
                }
                if value == 0 {
                    // Optimization: `x | 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_xor(&mut self) -> Self::Output {
        self.translate_binary_commutative::<i64, i64>(
            Instruction::i64_bitxor,
            Instruction::i64_bitxor_imm16,
            wasm::i64_bitxor,
            |this, lhs, rhs| {
                if lhs == rhs {
                    // Optimization: `x ^ x` is always `0`
                    this.stack.push_const(0_i64);
                    return Ok(true);
                }
                Ok(false)
            },
            |this, reg: Reg, value: i64| {
                if value == 0 {
                    // Optimization: `x ^ 0` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_shl(&mut self) -> Self::Output {
        self.translate_shift::<i64>(
            Instruction::i64_shl,
            Instruction::i64_shl_by,
            Instruction::i64_shl_imm16,
            wasm::i64_shl,
            Self::no_custom_opt,
        )
    }

    fn visit_i64_shr_s(&mut self) -> Self::Output {
        self.translate_shift::<i64>(
            Instruction::i64_shr_s,
            Instruction::i64_shr_s_by,
            Instruction::i64_shr_s_imm16,
            wasm::i64_shr_s,
            |this, lhs: i64, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1 >> x` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_shr_u(&mut self) -> Self::Output {
        self.translate_shift::<i64>(
            Instruction::i64_shr_u,
            Instruction::i64_shr_u_by,
            Instruction::i64_shr_u_imm16,
            wasm::i64_shr_u,
            Self::no_custom_opt,
        )
    }

    fn visit_i64_rotl(&mut self) -> Self::Output {
        self.translate_shift::<i64>(
            Instruction::i64_rotl,
            Instruction::i64_rotl_by,
            Instruction::i64_rotl_imm16,
            wasm::i64_rotl,
            |this, lhs: i64, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1 >> x` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_i64_rotr(&mut self) -> Self::Output {
        self.translate_shift::<i64>(
            Instruction::i64_rotr,
            Instruction::i64_rotr_by,
            Instruction::i64_rotr_imm16,
            wasm::i64_rotr,
            |this, lhs: i64, _rhs: Reg| {
                if lhs == -1 {
                    // Optimization: `-1 >> x` is always `-1` for every valid `x`
                    this.stack.push_const(lhs);
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_abs(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_abs, wasm::f32_abs)
    }

    fn visit_f32_neg(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_neg, wasm::f32_neg)
    }

    fn visit_f32_ceil(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_ceil, wasm::f32_ceil)
    }

    fn visit_f32_floor(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_floor, wasm::f32_floor)
    }

    fn visit_f32_trunc(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_trunc, wasm::f32_trunc)
    }

    fn visit_f32_nearest(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_nearest, wasm::f32_nearest)
    }

    fn visit_f32_sqrt(&mut self) -> Self::Output {
        self.translate_unary::<f32, f32>(Instruction::f32_sqrt, wasm::f32_sqrt)
    }

    fn visit_f32_add(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, f32>(
            Instruction::f32_add,
            wasm::f32_add,
            Self::no_custom_opt,
            Self::no_custom_opt::<Reg, f32>,
        )
    }

    fn visit_f32_sub(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, f32>(
            Instruction::f32_sub,
            wasm::f32_sub,
            Self::no_custom_opt,
            Self::no_custom_opt::<Reg, f32>,
            // Unfortunately we cannot optimize for the case that `lhs == 0.0`
            // since the Wasm specification mandates different behavior in
            // dependence of `rhs` which we do not know at this point.
            Self::no_custom_opt,
        )
    }

    fn visit_f32_mul(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, f32>(
            Instruction::f32_mul,
            wasm::f32_mul,
            Self::no_custom_opt,
            // Unfortunately we cannot apply `x * 0` or `0 * x` optimizations
            // since Wasm mandates different behaviors if `x` is infinite or
            // NaN in these cases.
            Self::no_custom_opt,
        )
    }

    fn visit_f32_div(&mut self) -> Self::Output {
        self.translate_fbinary::<f32, f32>(
            Instruction::f32_div,
            wasm::f32_div,
            Self::no_custom_opt,
            Self::no_custom_opt,
            Self::no_custom_opt,
        )
    }

    fn visit_f32_min(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, f32>(
            Instruction::f32_min,
            wasm::f32_min,
            Self::no_custom_opt,
            |this, reg: Reg, value: f32| {
                if value.is_infinite() && value.is_sign_positive() {
                    // Optimization: `min(x, +inf)` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_max(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f32, f32>(
            Instruction::f32_max,
            wasm::f32_max,
            Self::no_custom_opt,
            |this, reg: Reg, value: f32| {
                if value.is_infinite() && value.is_sign_negative() {
                    // Optimization: `max(x, -inf)` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f32_copysign(&mut self) -> Self::Output {
        self.translate_fcopysign::<f32>(
            Instruction::f32_copysign,
            Instruction::f32_copysign_imm,
            wasm::f32_copysign,
        )
    }

    fn visit_f64_abs(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_abs, wasm::f64_abs)
    }

    fn visit_f64_neg(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_neg, wasm::f64_neg)
    }

    fn visit_f64_ceil(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_ceil, wasm::f64_ceil)
    }

    fn visit_f64_floor(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_floor, wasm::f64_floor)
    }

    fn visit_f64_trunc(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_trunc, wasm::f64_trunc)
    }

    fn visit_f64_nearest(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_nearest, wasm::f64_nearest)
    }

    fn visit_f64_sqrt(&mut self) -> Self::Output {
        self.translate_unary::<f64, f64>(Instruction::f64_sqrt, wasm::f64_sqrt)
    }

    fn visit_f64_add(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, f64>(
            Instruction::f64_add,
            wasm::f64_add,
            Self::no_custom_opt,
            Self::no_custom_opt::<Reg, f64>,
        )
    }

    fn visit_f64_sub(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, f64>(
            Instruction::f64_sub,
            wasm::f64_sub,
            Self::no_custom_opt,
            Self::no_custom_opt::<Reg, f64>,
            // Unfortunately we cannot optimize for the case that `lhs == 0.0`
            // since the Wasm specification mandates different behavior in
            // dependence of `rhs` which we do not know at this point.
            Self::no_custom_opt,
        )
    }

    fn visit_f64_mul(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, f64>(
            Instruction::f64_mul,
            wasm::f64_mul,
            Self::no_custom_opt,
            // Unfortunately we cannot apply `x * 0` or `0 * x` optimizations
            // since Wasm mandates different behaviors if `x` is infinite or
            // NaN in these cases.
            Self::no_custom_opt,
        )
    }

    fn visit_f64_div(&mut self) -> Self::Output {
        self.translate_fbinary::<f64, f64>(
            Instruction::f64_div,
            wasm::f64_div,
            Self::no_custom_opt,
            Self::no_custom_opt,
            Self::no_custom_opt,
        )
    }

    fn visit_f64_min(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, f64>(
            Instruction::f64_min,
            wasm::f64_min,
            Self::no_custom_opt,
            |this, reg: Reg, value: f64| {
                if value.is_infinite() && value.is_sign_positive() {
                    // Optimization: `min(x, +inf)` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_max(&mut self) -> Self::Output {
        self.translate_fbinary_commutative::<f64, f64>(
            Instruction::f64_max,
            wasm::f64_max,
            Self::no_custom_opt,
            |this, reg: Reg, value: f64| {
                if value.is_infinite() && value.is_sign_negative() {
                    // Optimization: `min(x, +inf)` is same as `x`
                    this.stack.push_register(reg)?;
                    return Ok(true);
                }
                Ok(false)
            },
        )
    }

    fn visit_f64_copysign(&mut self) -> Self::Output {
        self.translate_fcopysign::<f64>(
            Instruction::f64_copysign,
            Instruction::f64_copysign_imm,
            wasm::f64_copysign,
        )
    }

    fn visit_i32_wrap_i64(&mut self) -> Self::Output {
        self.translate_unary::<i64, i32>(Instruction::i32_wrap_i64, wasm::i32_wrap_i64)
    }

    fn visit_i32_trunc_f32_s(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f32, i32>(
            Instruction::i32_trunc_f32_s,
            wasm::i32_trunc_f32_s,
        )
    }

    fn visit_i32_trunc_f32_u(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f32, u32>(
            Instruction::i32_trunc_f32_u,
            wasm::i32_trunc_f32_u,
        )
    }

    fn visit_i32_trunc_f64_s(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f64, i32>(
            Instruction::i32_trunc_f64_s,
            wasm::i32_trunc_f64_s,
        )
    }

    fn visit_i32_trunc_f64_u(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f64, u32>(
            Instruction::i32_trunc_f64_u,
            wasm::i32_trunc_f64_u,
        )
    }

    fn visit_i64_extend_i32_s(&mut self) -> Self::Output {
        self.translate_unary::<i32, i64>(Instruction::i64_extend32_s, wasm::i64_extend_i32_s)
    }

    fn visit_i64_extend_i32_u(&mut self) -> Self::Output {
        self.translate_reinterpret(wasm::i64_extend_i32_u)
    }

    fn visit_i64_trunc_f32_s(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f32, i64>(
            Instruction::i64_trunc_f32_s,
            wasm::i64_trunc_f32_s,
        )
    }

    fn visit_i64_trunc_f32_u(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f32, u64>(
            Instruction::i64_trunc_f32_u,
            wasm::i64_trunc_f32_u,
        )
    }

    fn visit_i64_trunc_f64_s(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f64, i64>(
            Instruction::i64_trunc_f64_s,
            wasm::i64_trunc_f64_s,
        )
    }

    fn visit_i64_trunc_f64_u(&mut self) -> Self::Output {
        self.translate_unary_fallible::<f64, u64>(
            Instruction::i64_trunc_f64_u,
            wasm::i64_trunc_f64_u,
        )
    }

    fn visit_f32_convert_i32_s(&mut self) -> Self::Output {
        self.translate_unary::<i32, f32>(Instruction::f32_convert_i32_s, wasm::f32_convert_i32_s)
    }

    fn visit_f32_convert_i32_u(&mut self) -> Self::Output {
        self.translate_unary::<u32, f32>(Instruction::f32_convert_i32_u, wasm::f32_convert_i32_u)
    }

    fn visit_f32_convert_i64_s(&mut self) -> Self::Output {
        self.translate_unary::<i64, f32>(Instruction::f32_convert_i64_s, wasm::f32_convert_i64_s)
    }

    fn visit_f32_convert_i64_u(&mut self) -> Self::Output {
        self.translate_unary::<u64, f32>(Instruction::f32_convert_i64_u, wasm::f32_convert_i64_u)
    }

    fn visit_f32_demote_f64(&mut self) -> Self::Output {
        self.translate_unary::<f64, f32>(Instruction::f32_demote_f64, wasm::f32_demote_f64)
    }

    fn visit_f64_convert_i32_s(&mut self) -> Self::Output {
        self.translate_unary::<i32, f64>(Instruction::f64_convert_i32_s, wasm::f64_convert_i32_s)
    }

    fn visit_f64_convert_i32_u(&mut self) -> Self::Output {
        self.translate_unary::<u32, f64>(Instruction::f64_convert_i32_u, wasm::f64_convert_i32_u)
    }

    fn visit_f64_convert_i64_s(&mut self) -> Self::Output {
        self.translate_unary::<i64, f64>(Instruction::f64_convert_i64_s, wasm::f64_convert_i64_s)
    }

    fn visit_f64_convert_i64_u(&mut self) -> Self::Output {
        self.translate_unary::<u64, f64>(Instruction::f64_convert_i64_u, wasm::f64_convert_i64_u)
    }

    fn visit_f64_promote_f32(&mut self) -> Self::Output {
        self.translate_unary::<f32, f64>(Instruction::f64_promote_f32, wasm::f64_promote_f32)
    }

    fn visit_i32_reinterpret_f32(&mut self) -> Self::Output {
        self.translate_reinterpret(wasm::i32_reinterpret_f32)
    }

    fn visit_i64_reinterpret_f64(&mut self) -> Self::Output {
        self.translate_reinterpret(wasm::i64_reinterpret_f64)
    }

    fn visit_f32_reinterpret_i32(&mut self) -> Self::Output {
        self.translate_reinterpret(wasm::f32_reinterpret_i32)
    }

    fn visit_f64_reinterpret_i64(&mut self) -> Self::Output {
        self.translate_reinterpret(wasm::f64_reinterpret_i64)
    }

    fn visit_i32_extend8_s(&mut self) -> Self::Output {
        self.translate_unary::<i32, i32>(Instruction::i32_extend8_s, wasm::i32_extend8_s)
    }

    fn visit_i32_extend16_s(&mut self) -> Self::Output {
        self.translate_unary::<i32, i32>(Instruction::i32_extend16_s, wasm::i32_extend16_s)
    }

    fn visit_i64_extend8_s(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_extend8_s, wasm::i64_extend8_s)
    }

    fn visit_i64_extend16_s(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_extend16_s, wasm::i64_extend16_s)
    }

    fn visit_i64_extend32_s(&mut self) -> Self::Output {
        self.translate_unary::<i64, i64>(Instruction::i64_extend32_s, wasm::i64_extend32_s)
    }

    fn visit_i32_trunc_sat_f32_s(&mut self) -> Self::Output {
        self.translate_unary::<f32, i32>(
            Instruction::i32_trunc_sat_f32_s,
            wasm::i32_trunc_sat_f32_s,
        )
    }

    fn visit_i32_trunc_sat_f32_u(&mut self) -> Self::Output {
        self.translate_unary::<f32, u32>(
            Instruction::i32_trunc_sat_f32_u,
            wasm::i32_trunc_sat_f32_u,
        )
    }

    fn visit_i32_trunc_sat_f64_s(&mut self) -> Self::Output {
        self.translate_unary::<f64, i32>(
            Instruction::i32_trunc_sat_f64_s,
            wasm::i32_trunc_sat_f64_s,
        )
    }

    fn visit_i32_trunc_sat_f64_u(&mut self) -> Self::Output {
        self.translate_unary::<f64, u32>(
            Instruction::i32_trunc_sat_f64_u,
            wasm::i32_trunc_sat_f64_u,
        )
    }

    fn visit_i64_trunc_sat_f32_s(&mut self) -> Self::Output {
        self.translate_unary::<f32, i64>(
            Instruction::i64_trunc_sat_f32_s,
            wasm::i64_trunc_sat_f32_s,
        )
    }

    fn visit_i64_trunc_sat_f32_u(&mut self) -> Self::Output {
        self.translate_unary::<f32, u64>(
            Instruction::i64_trunc_sat_f32_u,
            wasm::i64_trunc_sat_f32_u,
        )
    }

    fn visit_i64_trunc_sat_f64_s(&mut self) -> Self::Output {
        self.translate_unary::<f64, i64>(
            Instruction::i64_trunc_sat_f64_s,
            wasm::i64_trunc_sat_f64_s,
        )
    }

    fn visit_i64_trunc_sat_f64_u(&mut self) -> Self::Output {
        self.translate_unary::<f64, u64>(
            Instruction::i64_trunc_sat_f64_u,
            wasm::i64_trunc_sat_f64_u,
        )
    }

    fn visit_memory_init(&mut self, data_index: u32, mem: u32) -> Self::Output {
        bail_unreachable!(self);
        let memory = index::Memory::from(mem);
        let (dst, src, len) = self.stack.pop3();
        let dst = self.stack.provider2reg(&dst)?;
        let src = self.stack.provider2reg(&src)?;
        let len = <Provider<Const16<u32>>>::new(len, &mut self.stack)?;
        let instr = match len {
            Provider::Register(len) => Instruction::memory_init(dst, src, len),
            Provider::Const(len) => Instruction::memory_init_imm(dst, src, len),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::memory_index(memory))?;
        self.instr_encoder
            .append_instr(Instruction::data_index(data_index))?;
        Ok(())
    }

    fn visit_data_drop(&mut self, data_index: u32) -> Self::Output {
        bail_unreachable!(self);
        self.push_fueled_instr(
            Instruction::data_drop(data_index),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_memory_copy(&mut self, dst_mem: u32, src_mem: u32) -> Self::Output {
        bail_unreachable!(self);
        let dst_memory = index::Memory::from(dst_mem);
        let src_memory = index::Memory::from(src_mem);

        let (dst, src, len) = self.stack.pop3();

        let dst_memory_type = *self.module.get_type_of_memory(MemoryIdx::from(dst_mem));
        let src_memory_type = *self.module.get_type_of_memory(MemoryIdx::from(src_mem));
        let min_index_ty = dst_memory_type.index_ty().min(&src_memory_type.index_ty());
        let dst = self.stack.provider2reg(&dst)?;
        let src = self.stack.provider2reg(&src)?;
        let len = self.as_index_type_const16(len, min_index_ty)?;

        let instr = match len {
            Provider::Register(len) => Instruction::memory_copy(dst, src, len),
            Provider::Const(len) => Instruction::memory_copy_imm(dst, src, len),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::memory_index(dst_memory))?;
        self.instr_encoder
            .append_instr(Instruction::memory_index(src_memory))?;
        Ok(())
    }

    fn visit_memory_fill(&mut self, mem: u32) -> Self::Output {
        bail_unreachable!(self);
        let memory = index::Memory::from(mem);
        let memory_type = *self.module.get_type_of_memory(MemoryIdx::from(mem));
        let (dst, value, len) = self.stack.pop3();
        let dst = self.stack.provider2reg(&dst)?;
        let value = value.map_const(|value| u32::from(value) as u8);
        let len = self.as_index_type_const16(len, memory_type.index_ty())?;
        let instr = match (value, len) {
            (Provider::Register(value), Provider::Register(len)) => {
                Instruction::memory_fill(dst, value, len)
            }
            (Provider::Register(value), Provider::Const(len)) => {
                Instruction::memory_fill_exact(dst, value, len)
            }
            (Provider::Const(value), Provider::Register(len)) => {
                Instruction::memory_fill_imm(dst, value, len)
            }
            (Provider::Const(value), Provider::Const(len)) => {
                Instruction::memory_fill_imm_exact(dst, value, len)
            }
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::memory_index(memory))?;
        Ok(())
    }

    fn visit_table_init(&mut self, elem_index: u32, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let (dst, src, len) = self.stack.pop3();
        let dst = self.stack.provider2reg(&dst)?;
        let src = self.stack.provider2reg(&src)?;
        let len = <Provider<Const16<u32>>>::new(len, &mut self.stack)?;
        let instr = match len {
            Provider::Register(len) => Instruction::table_init(dst, src, len),
            Provider::Const(len) => Instruction::table_init_imm(dst, src, len),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(table))?;
        self.instr_encoder
            .append_instr(Instruction::elem_index(elem_index))?;
        Ok(())
    }

    fn visit_elem_drop(&mut self, elem_index: u32) -> Self::Output {
        bail_unreachable!(self);
        self.push_fueled_instr(
            Instruction::elem_drop(elem_index),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_table_copy(&mut self, dst_table: u32, src_table: u32) -> Self::Output {
        bail_unreachable!(self);
        let (dst, src, len) = self.stack.pop3();
        let dst_table_type = *self.module.get_type_of_table(TableIdx::from(dst_table));
        let src_table_type = *self.module.get_type_of_table(TableIdx::from(src_table));
        let min_index_ty = dst_table_type.index_ty().min(&src_table_type.index_ty());
        let dst = self.stack.provider2reg(&dst)?;
        let src = self.stack.provider2reg(&src)?;
        let len = self.as_index_type_const16(len, min_index_ty)?;
        let instr = match len {
            Provider::Register(len) => Instruction::table_copy(dst, src, len),
            Provider::Const(len) => Instruction::table_copy_imm(dst, src, len),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(dst_table))?;
        self.instr_encoder
            .append_instr(Instruction::table_index(src_table))?;
        Ok(())
    }

    fn visit_table_fill(&mut self, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let (dst, value, len) = self.stack.pop3();
        let table_type = *self.module.get_type_of_table(TableIdx::from(table));
        let dst = self.stack.provider2reg(&dst)?;
        let len = self.as_index_type_const16(len, table_type.index_ty())?;
        let value = match value {
            TypedProvider::Register(value) => value,
            TypedProvider::Const(value) => self.stack.alloc_const(value)?,
        };
        let instr = match len {
            Provider::Register(len) => Instruction::table_fill(dst, len, value),
            Provider::Const(len) => Instruction::table_fill_imm(dst, len, value),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(table))?;
        Ok(())
    }

    fn visit_table_get(&mut self, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let table_type = *self.module.get_type_of_table(TableIdx::from(table));
        let index = self.stack.pop();
        let result = self.stack.push_dynamic()?;
        let index = self.as_index_type_const32(index, table_type.index_ty())?;
        let instr = match index {
            Provider::Register(index) => Instruction::table_get(result, index),
            Provider::Const(index) => Instruction::table_get_imm(result, index),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(table))?;
        Ok(())
    }

    fn visit_table_set(&mut self, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let table_type = *self.module.get_type_of_table(TableIdx::from(table));
        let (index, value) = self.stack.pop2();
        let index = self.as_index_type_const32(index, table_type.index_ty())?;
        let value = match value {
            TypedProvider::Register(value) => value,
            TypedProvider::Const(value) => self.stack.alloc_const(value)?,
        };
        let instr = match index {
            Provider::Register(index) => Instruction::table_set(index, value),
            Provider::Const(index) => Instruction::table_set_at(value, index),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(table))?;
        Ok(())
    }

    fn visit_table_grow(&mut self, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let table_type = *self.module.get_type_of_table(TableIdx::from(table));
        let (value, delta) = self.stack.pop2();
        let delta = self.as_index_type_const16(delta, table_type.index_ty())?;
        if let Provider::Const(delta) = delta {
            if u64::from(delta) == 0 {
                // Case: growing by 0 elements.
                //
                // Since `table.grow` returns the `table.size` before the
                // operation a `table.grow` with `delta` of 0 can be translated
                // as `table.size` instruction instead.
                let result = self.stack.push_dynamic()?;
                self.push_fueled_instr(
                    Instruction::table_size(result, table),
                    FuelCostsProvider::instance,
                )?;
                return Ok(());
            }
        }
        let value = match value {
            TypedProvider::Register(value) => value,
            TypedProvider::Const(value) => self.stack.alloc_const(value)?,
        };
        let result = self.stack.push_dynamic()?;
        let instr = match delta {
            Provider::Register(delta) => Instruction::table_grow(result, delta, value),
            Provider::Const(delta) => Instruction::table_grow_imm(result, delta, value),
        };
        self.push_fueled_instr(instr, FuelCostsProvider::instance)?;
        self.instr_encoder
            .append_instr(Instruction::table_index(table))?;
        Ok(())
    }

    fn visit_table_size(&mut self, table: u32) -> Self::Output {
        bail_unreachable!(self);
        let result = self.stack.push_dynamic()?;
        self.push_fueled_instr(
            Instruction::table_size(result, table),
            FuelCostsProvider::instance,
        )?;
        Ok(())
    }

    fn visit_i64_add128(&mut self) -> Self::Output {
        self.translate_i64_binop128(Instruction::i64_add128, wasm::i64_add128)
    }

    fn visit_i64_sub128(&mut self) -> Self::Output {
        self.translate_i64_binop128(Instruction::i64_sub128, wasm::i64_sub128)
    }

    fn visit_i64_mul_wide_s(&mut self) -> Self::Output {
        self.translate_i64_mul_wide_sx(Instruction::i64_mul_wide_s, wasm::i64_mul_wide_s, true)
    }

    fn visit_i64_mul_wide_u(&mut self) -> Self::Output {
        self.translate_i64_mul_wide_sx(Instruction::i64_mul_wide_u, wasm::i64_mul_wide_u, false)
    }
}
