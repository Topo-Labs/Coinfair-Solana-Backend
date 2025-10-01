use anchor_lang::{
    error::{Error, ErrorCode},
    solana_program::{account_info::AccountInfo, pubkey::Pubkey},
    Key, Owner, Result, ToAccountInfos, ZeroCopy,
};
use arrayref::array_ref;
use std::cell::{Ref, RefMut};
use std::marker::PhantomData;
use std::mem;
use std::ops::DerefMut;

#[derive(Clone)]
pub struct AccountLoad<'info, T: ZeroCopy + Owner> {
    acc_info: AccountInfo<'info>,
    phantom: PhantomData<&'info T>,
}

impl<'info, T: ZeroCopy + Owner> AccountLoad<'info, T> {
    fn new(acc_info: AccountInfo<'info>) -> AccountLoad<'info, T> {
        Self {
            acc_info,
            phantom: PhantomData,
        }
    }

    /// 从先前初始化的账户构造一个新的 `Loader`。
    #[inline(never)]
    pub fn try_from(acc_info: &AccountInfo<'info>) -> Result<AccountLoad<'info, T>> {
        if acc_info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram).with_pubkeys((*acc_info.owner, T::owner())));
        }
        let data: &[u8] = &acc_info.try_borrow_data()?;
        if data.len() < T::DISCRIMINATOR.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }
        // 鉴别器必须匹配。
        let disc_bytes = array_ref![data, 0, 8];
        if disc_bytes != &T::DISCRIMINATOR {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Ok(AccountLoad::new(acc_info.clone()))
    }

    /// 从未初始化的账户构造一个新的 `Loader`。
    #[inline(never)]
    pub fn try_from_unchecked(_program_id: &Pubkey, acc_info: &AccountInfo<'info>) -> Result<AccountLoad<'info, T>> {
        if acc_info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram).with_pubkeys((*acc_info.owner, T::owner())));
        }
        Ok(AccountLoad::new(acc_info.clone()))
    }

    /// 返回用于读取或写入账户数据结构的 `RefMut`。
    /// 应该只在账户被初始化时调用一次。
    pub fn load_init(&self) -> Result<RefMut<T>> {
        // AccountInfo API 允许您在账户不可写时借用可变引用，
        // 因此添加此检查以提供更好的开发体验。
        if !self.acc_info.is_writable {
            return Err(ErrorCode::AccountNotMutable.into());
        }

        let mut data = self.acc_info.try_borrow_mut_data()?;

        // 鉴别器应该为零，因为我们正在初始化。
        let mut disc_bytes = [0u8; 8];
        disc_bytes.copy_from_slice(&data[..8]);
        let discriminator = u64::from_le_bytes(disc_bytes);
        if discriminator != 0 {
            return Err(ErrorCode::AccountDiscriminatorAlreadySet.into());
        }

        // 写入鉴别器
        data[..8].copy_from_slice(&T::DISCRIMINATOR);

        Ok(RefMut::map(data, |data| {
            bytemuck::from_bytes_mut(&mut data.deref_mut()[8..mem::size_of::<T>() + 8])
        }))
    }

    /// 直接返回用于读取或写入账户数据结构的 `RefMut`。
    /// 无需将 AccountInfo 转换为 AccountLoad。
    /// 因此需要检查所有者
    pub fn load_data_mut<'a>(acc_info: &'a AccountInfo) -> Result<RefMut<'a, T>> {
        if acc_info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram).with_pubkeys((*acc_info.owner, T::owner())));
        }
        if !acc_info.is_writable {
            return Err(ErrorCode::AccountNotMutable.into());
        }

        let data = acc_info.try_borrow_mut_data()?;
        if data.len() < T::DISCRIMINATOR.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let disc_bytes = array_ref![data, 0, 8];
        if disc_bytes != &T::DISCRIMINATOR {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Ok(RefMut::map(data, |data| {
            bytemuck::from_bytes_mut(&mut data.deref_mut()[8..mem::size_of::<T>() + 8])
        }))
    }

    /// 返回用于读取账户数据结构的 Ref。
    pub fn load(&self) -> Result<Ref<T>> {
        let data = self.acc_info.try_borrow_data()?;
        if data.len() < T::DISCRIMINATOR.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let disc_bytes = array_ref![data, 0, 8];
        if disc_bytes != &T::DISCRIMINATOR {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Ok(Ref::map(data, |data| {
            bytemuck::from_bytes(&data[8..mem::size_of::<T>() + 8])
        }))
    }

    /// 返回用于读取或写入账户数据结构的 `RefMut`。
    pub fn load_mut(&self) -> Result<RefMut<T>> {
        // AccountInfo API 允许您在账户不可写时借用可变引用，
        // 因此添加此检查以提供更好的开发体验。
        if !self.acc_info.is_writable {
            return Err(ErrorCode::AccountNotMutable.into());
        }

        let data = self.acc_info.try_borrow_mut_data()?;
        if data.len() < T::DISCRIMINATOR.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let disc_bytes = array_ref![data, 0, 8];
        if disc_bytes != &T::DISCRIMINATOR {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Ok(RefMut::map(data, |data| {
            bytemuck::from_bytes_mut(&mut data.deref_mut()[8..mem::size_of::<T>() + 8])
        }))
    }
}

impl<'info, T: ZeroCopy + Owner> AsRef<AccountInfo<'info>> for AccountLoad<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        &self.acc_info
    }
}
impl<'info, T: ZeroCopy + Owner> ToAccountInfos<'info> for AccountLoad<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.acc_info.clone()]
    }
}

impl<'info, T: ZeroCopy + Owner> Key for AccountLoad<'info, T> {
    fn key(&self) -> Pubkey {
        *self.acc_info.key
    }
}
