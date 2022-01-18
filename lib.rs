#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod erc20 {
    use ink_storage::{collections::HashMap, lazy::Lazy};
    /// Erc20 的存储结构体
    #[ink(storage)]
    pub struct Erc20 {
        /// total
        total_supply: Lazy<Balance>,
        balances: HashMap<AccountId, Balance>,
        allowances: HashMap<(AccountId, AccountId), Balance>,
    }
    /// 事件定义
    #[ink(event)]
    pub struct Transfer {
        // #[ink(topic)] 用于标记希望索引的项目, 以便后续搜索使用
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        value: Balance,
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        owner: AccountId,
        #[ink(topic)]
        spender: AccountId,
        value: Balance,
    }
    // Error 结构体需要满足的trait bound, 这些trait已经默认引入了
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        InsufficientBalance,
        InsufficientAllowance,
    }

    // 用一个Result类包裹Error
    pub type Result<T> = core::result::Result<T, Error>;

    impl Erc20 {
        //初始化构造函数
        #[ink(constructor)]
        pub fn new(supply: Balance) -> Self {
            let caller = Self::env().caller();
            let mut balances = HashMap::new();
            balances.insert(caller, supply);

            Self::env().emit_event(Transfer {
                from: None,
                to: Some(caller),
                value: supply,
            });

            Self {
                total_supply: Lazy::new(supply),
                balances,
                allowances: HashMap::new(),
            }
        }
        // 各种get函数
        #[ink(message)]
        pub fn total_supply(&self) -> Balance {
            *self.total_supply
        }

        #[ink(message)]
        pub fn balance_of(&self, who: AccountId) -> Balance {
            self.balances.get(&who).copied().unwrap_or_default()
        }

        #[ink(message)]
        pub fn allowance(&self, owner: AccountId, spender: AccountId) -> Balance {
            self.allowances
                .get(&(owner, spender))
                .copied()
                .unwrap_or_default()
        }

        //transfer / approve / transfer_from  等会修改状态的方法, 第一参数必须为 &mut self
        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, value: Balance) -> Result<()> {
            let from = self.env().caller();

            self.inner_transfer(from, to, value)
        }

        #[ink(message)]
        pub fn approve(&mut self, to: AccountId, value: Balance) -> Result<()> {
            let owner = self.env().caller();

            self.allowances.insert((owner, to), value);
            self.env().emit_event(Approval {
                owner,
                spender: to,
                value,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> Result<()> {
            let caller = self.env().caller();
            let allowance = self.allowance(from, caller);
            if allowance < value {
                return Err(Error::InsufficientAllowance);
            }

            self.inner_transfer(from, to, value)?;
            self.allowances.insert((from, caller), allowance - value);

            Ok(())
        }
        //私有helper方法
        fn inner_transfer(&mut self, from: AccountId, to: AccountId, value: Balance) -> Result<()> {
            let from_balance = self.balance_of(from);
            if from_balance < value {
                return Err(Error::InsufficientBalance);
            }

            self.balances.insert(from, from_balance - value);
            let to_balance = self.balance_of(to);
            self.balances.insert(to, to_balance + value);
            self.env().emit_event(Transfer {
                from: Some(from),
                to: Some(to),
                value,
            });

            Ok(())
        }
    }
    //测试模块, 重点参考https://paritytech.github.io/ink/ink_env/test/index.html 文档
    //和https://paritytech.github.io/ink-docs/basics/contract-testing
    #[cfg(test)]
    mod tests {
        use super::*;

        // 将事件收敛为一个类型, 参考 https://paritytech.github.io/ink/ink_lang/reflect/trait.ContractEventBase.html
        // ink_lang::reflect 模块是合约的静态反射, 用来检查合约编译时信息
        type Event = <Erc20 as ::ink_lang::reflect::ContractEventBase>::Type;

        use ink_lang as ink;

        struct PrefixedValue<'a, 'b, T> {
            pub prefix: &'a [u8],
            pub value: &'b T,
        }

        impl<X> scale::Encode for PrefixedValue<'_, '_, X>
        where
            X: scale::Encode,
        {
            #[inline]
            fn size_hint(&self) -> usize {
                self.prefix.size_hint() + self.value.size_hint()
            }

            #[inline]
            fn encode_to<T: scale::Output + ?Sized>(&self, dest: &mut T) {
                self.prefix.encode_to(dest);
                self.value.encode_to(dest);
            }
        }

        #[cfg(test)]
        fn encoded_into_hash<T>(entity: &T) -> Hash
        where
            T: scale::Encode,
        {
            use ink_env::{
                hash::{Blake2x256, CryptoHash, HashOutput},
                Clear,
            };
            let mut result = Hash::clear();
            let len_result = result.as_ref().len();
            let encoded = entity.encode();
            let len_encoded = encoded.len();
            if len_encoded <= len_result {
                result.as_mut()[..len_encoded].copy_from_slice(&encoded);
                return result;
            }
            let mut hash_output = <<Blake2x256 as HashOutput>::Type as Default>::default();
            <Blake2x256 as CryptoHash>::hash(&encoded, &mut hash_output);
            let copy_len = core::cmp::min(hash_output.len(), len_result);
            result.as_mut()[0..copy_len].copy_from_slice(&hash_output[0..copy_len]);
            result
        }

        fn assert_transfer_event(
            event: &ink_env::test::EmittedEvent, // 参考https://paritytech.github.io/ink/ink_env/test/struct.EmittedEvent.html
            expected_from: Option<AccountId>,
            expected_to: Option<AccountId>,
            expected_value: Balance,
        ) {
            let decode_event = <Event as scale::Decode>::decode(&mut &event.data[..])
                .expect("encountered invalid contract event data buffer");
            if let Event::Transfer(Transfer { from, to, value }) = decode_event {
                assert_eq!(from, expected_from, "encountered invalid transfer.from");
                assert_eq!(to, expected_to, "encountered invalid transfer.to");
                assert_eq!(value, expected_value, "encountered invalid transfer.value");
            } else {
                panic!("encountered unexpected evnet kind: expect a Transfer event")
            }
            let expected_topics = vec![
                encoded_into_hash(&PrefixedValue {
                    value: b"Erc20::Transfer",
                    prefix: b"",
                }),
                encoded_into_hash(&PrefixedValue {
                    prefix: b"Erc20::Transfer::from",
                    value: &expected_from,
                }),
                encoded_into_hash(&PrefixedValue {
                    prefix: b"Erc20::Transfer::to",
                    value: &expected_to,
                }),
                encoded_into_hash(&PrefixedValue {
                    prefix: b"Erc20::Transfer::value",
                    value: &expected_value,
                }),
            ];

            for (n, (actual_topic, expect_topic)) in
                event.topics.iter().zip(expected_topics).enumerate()
            {
                let topic = actual_topic
                    .decode::<Hash>()
                    .expect("encountered invalid topic encoding");
                assert_eq!(topic, expect_topic, "encountered invalid topic at {}", n);
            }
        }
        #[ink::test]
        fn new_works() {
            let _erc20 = Erc20::new(100);

            let emit_events = ink_env::test::recorded_events().collect::<Vec<_>>();

            assert_eq!(1, emit_events.len());

            assert_transfer_event(
                &emit_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );
        }

        #[ink::test]
        fn total_supply_works() {
            let erc20 = Erc20::new(100);

            let emit_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_transfer_event(
                &emit_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );

            assert_eq!(erc20.total_supply(), 100);
        }

        #[ink::test]
        fn balance_of_works() {
            let erc20 = Erc20::new(100);

            let emit_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_transfer_event(
                &emit_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );

            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                .expect("Cannot get accounts");

            assert_eq!(erc20.balance_of(accounts.alice), 100);
            assert_eq!(erc20.balance_of(accounts.bob), 0);
        }

        #[ink::test]
        fn transfer_works() {
            // 此处小坑, 一定要定义为mut
            let mut erc20 = Erc20::new(100);

            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                .expect("Cannot get accounts");

            assert_eq!(erc20.balance_of(accounts.alice), 100);
            assert_eq!(erc20.balance_of(accounts.bob), 0);

            //transfer 10 to bob
            assert_eq!(erc20.transfer(accounts.bob, 10), Ok(()));

            assert_eq!(erc20.balance_of(accounts.alice), 90);
            assert_eq!(erc20.balance_of(accounts.bob), 10);

            let emit_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emit_events.len(), 2);
            assert_transfer_event(
                &emit_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );
            assert_transfer_event(
                &emit_events[1],
                Some(AccountId::from([0x01; 32])),
                Some(AccountId::from([0x02; 32])),
                10,
            );
        }

        #[ink::test]
        fn trasfer_fails_when_not_enough_balance() {
            let mut erc20 = Erc20::new(100);

            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                .expect("Cannot get accounts");

            assert_eq!(erc20.balance_of(accounts.bob), 0);

            let callee = ink_env::account_id::<ink_env::DefaultEnvironment>();

            let mut data = ink_env::test::CallData::new(ink_env::call::Selector::new([0x00; 4])); // 表示4th 的call方法, 即transfer

            data.push_arg(&accounts.bob);

            ink_env::test::push_execution_context::<ink_env::DefaultEnvironment>(
                accounts.bob,
                callee,
                1000000,
                1000000,
                data,
            );

            assert_eq!(
                erc20.transfer(accounts.eve, 10),
                Err(Error::InsufficientBalance)
            );

            assert_eq!(erc20.balance_of(accounts.alice), 100);
            assert_eq!(erc20.balance_of(accounts.bob), 0);
            assert_eq!(erc20.balance_of(accounts.eve), 0);

            let emit_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emit_events.len(), 1);
            assert_transfer_event(
                &emit_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );
        }

        #[ink::test]
        fn transfer_from_works() {
            let mut erc20 = Erc20::new(100);
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                .expect("Cannot get accounts");

            assert_eq!(
                erc20.transfer_from(accounts.alice, accounts.eve, 10),
                Err(Error::InsufficientAllowance)
            );
            assert_eq!(erc20.approve(accounts.bob, 10), Ok(()));

            assert_eq!(ink_env::test::recorded_events().count(), 2);

            let callee = ink_env::account_id::<ink_env::DefaultEnvironment>();
            let mut data = ink_env::test::CallData::new(ink_env::call::Selector::new([0x00; 4])); 
            data.push_arg(&accounts.bob);
            ink_env::test::push_execution_context::<ink_env::DefaultEnvironment>(
                accounts.bob,
                callee,
                1000000,
                1000000,
                data,
            );

            assert_eq!(
                erc20.transfer_from(accounts.alice, accounts.eve, 10),
                Ok(())
            );
            assert_eq!(erc20.balance_of(accounts.eve), 10);

            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emitted_events.len(), 3);
            assert_transfer_event(
                &emitted_events[0],
                None,
                Some(AccountId::from([0x01; 32])),
                100,
            );
            assert_transfer_event(
                &emitted_events[2],
                Some(AccountId::from([0x01; 32])),
                Some(AccountId::from([0x05; 32])),
                10,
            );
        }

        #[ink::test]
        fn allowance_must_not_change_on_failed_transfer() {
            let mut erc20 = Erc20::new(100);
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>()
                .expect("Cannot get accounts");

            let alice_balance = erc20.balance_of(accounts.alice);
            let initial_allowance = alice_balance + 2;
            assert_eq!(erc20.approve(accounts.bob, initial_allowance), Ok(()));

            let callee = ink_env::account_id::<ink_env::DefaultEnvironment>();
            let mut data = ink_env::test::CallData::new(ink_env::call::Selector::new([0x00; 4])); 
            data.push_arg(&accounts.bob);
            ink_env::test::push_execution_context::<ink_env::DefaultEnvironment>(
                accounts.bob,
                callee,
                1000000,
                1000000,
                data,
            );

            let emitted_events_before = ink_env::test::recorded_events();
            assert_eq!(
                erc20.transfer_from(accounts.alice, accounts.eve, alice_balance + 1),
                Err(Error::InsufficientBalance)
            );
            assert_eq!(
                erc20.allowance(accounts.alice, accounts.bob),
                initial_allowance
            );
            let emitted_events_after = ink_env::test::recorded_events();
            assert_eq!(emitted_events_before.count(), emitted_events_after.count());
        }
    }
}
