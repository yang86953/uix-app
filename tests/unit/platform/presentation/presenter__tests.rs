use super::IPresenter;
use crate::core::{PresentCoherency, PresentDamage, PresentSurface};

// 只实现必须操作，用于验证 trait 的中立默认契约。
struct ContractPresenter;

impl IPresenter for ContractPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> crate::core::Result<()> {
        Ok(())
    }
}

#[test]
fn defaults_preserve_full_only_identity_contract() {
    let presenter = ContractPresenter;

    assert_eq!(presenter.present_coherency(), PresentCoherency::FullOnly);
    assert_eq!(
        presenter.present_surface(640, 360, 2.0),
        PresentSurface::identity(640, 360, 2.0, 0)
    );
    assert_eq!(presenter.present_image(), None);
}
