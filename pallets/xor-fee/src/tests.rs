// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::extension::ChargeTransactionPayment;
use crate::{mock::*, LiquidityInfo, XorToVal};
use common::balance;

use common::mock::{alice, bob};
use frame_support::error::BadOrigin;
use frame_support::traits::Currency;
use frame_support::weights::{Weight, WeightToFee};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::SignedExtension;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
use sp_runtime::{FixedPointNumber, FixedU128};

fn set_weight_to_fee_multiplier(mul: u64) {
    // Set WeightToFee multiplier to one to not affect the test
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(mul)
    ));
}

#[test]
fn weight_to_fee_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(100_000_000_000, 0)),
            balance!(0.7)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(500_000_000, 0)),
            balance!(0.0035)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(72_000_000, 0)),
            balance!(0.000504)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(210_200_000_000, 0)),
            balance!(1.4714)
        );
    });
}

#[test]
fn weight_to_fee_does_not_underflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        assert_eq!(XorFee::weight_to_fee(&Weight::zero()), 0);
    });
}

#[test]
fn weight_to_fee_does_not_overflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::MAX),
            129127208515966861305000000
        );
    });
}

#[test]
fn simple_update_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // Update from root
        set_weight_to_fee_multiplier(3);
        assert_eq!(XorFee::multiplier(), FixedU128::saturating_from_integer(3));
    });
}

#[test]
fn non_root_update_fails() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // We allow only root

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::signed(alice()), FixedU128::from(3)),
            BadOrigin
        );

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::none(), FixedU128::from(3)),
            BadOrigin
        );
    });
}

#[test]
fn it_works_postpone() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Postponed(who.clone()),
                Some(balance!(0.0007)),
            )
        );
        let _ = Balances::deposit_creating(&who, balance!(1000));
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info_from_weight(100.into()),
            &post_info_from_weight(100.into()),
            100,
            &Ok(()),
        )
        .unwrap();
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.3));
        assert_eq!(XorToVal::<Runtime>::get(), balance!(0.35));
    });
}

#[test]
fn it_fails_postpone() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_not_pay() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPaysNoAccountId::get();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(who.clone(), None),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_pays_no() {
    ExtBuilder::build().execute_with(|| {
        let who = bob();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_pays_no(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(who.clone(), None),
                None,
            )
        );
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_pays_no(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_post_info_pays_no() {
    ExtBuilder::build().execute_with(|| {
        let who = bob();
        let _ = Balances::deposit_creating(&who, balance!(1000));
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    who.clone(),
                    Some(pallet_balances::NegativeImbalance::new(balance!(0.7)))
                ),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.3));
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_pays_no(),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_postpone_with_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        let post_info = post_info_from_weight(100.into());
        let result = Ok(());
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(Balances::usable_balance_for_fees(&fee_source), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&who, &call, &info, len)
            .unwrap();
        let fee = XorFee::compute_fee(len as u32, &call, &info, 0).0;
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Postponed(fee_source.clone()),
                None,
            )
        );
        let _ = Balances::deposit_creating(&fee_source, balance!(1000));
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000)
        );
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info,
            &post_info,
            len,
            &result,
        )
        .unwrap();
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        assert_eq!(XorToVal::<Runtime>::get(), fee / 2);
    });
}

#[test]
fn it_works_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        let post_info = post_info_from_weight(100.into());
        let result = Ok(());
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let _ = Balances::deposit_creating(&fee_source, balance!(1000));
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000)
        );
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&who, &call, &info, len)
            .unwrap();
        let fee = XorFee::compute_fee(len as u32, &call, &info, 0).0;
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    fee_source.clone(),
                    Some(pallet_balances::NegativeImbalance::new(fee))
                ),
                None,
            )
        );
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info,
            &post_info,
            len,
            &result,
        )
        .unwrap();
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        assert_eq!(XorToVal::<Runtime>::get(), fee / 2);
    });
}

#[test]
fn it_fails_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        assert_eq!(Balances::usable_balance_for_fees(&fee_source), 0);
        let _ = Balances::deposit_creating(&who, balance!(1000));
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::new().pre_dispatch(&who, &call, &info, len),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
    });
}

#[test]
fn it_works_referrer_refund() {
    ExtBuilder::build().execute_with(|| {
        let who = GetReferalAccountId::get();
        let referrer = GetReferrerAccountId::get();
        let _ = Balances::deposit_creating(&who, balance!(1000));
        let _ = Balances::deposit_creating(&referrer, balance!(1000));
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    who.clone(),
                    Some(pallet_balances::NegativeImbalance::new(balance!(0.7)))
                ),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.3));
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info_from_weight(100.into()),
            &post_info_from_weight(100.into()),
            100,
            &Ok(()),
        )
        .unwrap();
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.3));
        assert_eq!(
            Balances::usable_balance_for_fees(&referrer),
            balance!(1000.07)
        );
        assert_eq!(XorToVal::<Runtime>::get(), balance!(0.35));
    });
}
