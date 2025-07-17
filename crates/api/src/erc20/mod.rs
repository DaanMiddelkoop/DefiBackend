use alloy::primitives::{Address, U256};
use alloy_sol_types::sol;
use providers::State;

sol! {
    "src/erc20/IERC20.sol"
}

pub struct ERC20 {
    pub address: Address,
}

impl ERC20 {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub async fn name(&self, state: &mut impl State) -> Result<String, String> {
        state.call::<IERC20::nameCall>(Address::ZERO, self.address, U256::ZERO, ()).await
    }

    pub async fn balance_of(&self, address: Address, state: &mut impl State) -> Result<U256, String> {
        state
            .call::<IERC20::balanceOfCall>(Address::ZERO, self.address, U256::ZERO, (address,))
            .await
    }
}
