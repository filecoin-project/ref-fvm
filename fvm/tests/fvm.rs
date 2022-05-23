mod default_kernel;
mod dummy;

use dummy::DummyExterns;

// TODO
// #[test]
// fn test_msg() -> anyhow::Result<()> {
//     let kern = TestingKernel::new(
//         DummyCallManager::new_stub(),
//         BlockRegistry::default(),
//         0,
//         0,
//         0,
//         0.into(),
//     );

//     kern.msg_receiver();
//     assert_eq!(0, kern.msg_receiver());

//     Ok(())
// }
