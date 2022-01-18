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

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
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
            let expected_topic = vec![
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
        }
    }
}
