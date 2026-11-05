import unittest

from tools import check_ipa

ENGINE = [
    "Payload/",
    "Payload/Horizon.app/",
    "Payload/Horizon.app/Horizon",
    "Payload/Horizon.app/Info.plist",
    "Payload/Horizon.app/PkgInfo",
    "Payload/Horizon.app/Assets.car",
    "Payload/Horizon.app/LaunchScreen.storyboardc/",
    "Payload/Horizon.app/LaunchScreen.storyboardc/Info.plist",
    "Payload/Horizon.app/en.lproj/",
    "Payload/Horizon.app/en.lproj/Localizable.strings",
    "Payload/Horizon.app/_CodeSignature/",
    "Payload/Horizon.app/_CodeSignature/CodeResources",
]


class IpaTests(unittest.TestCase):
    def test_engine_only_ipa_passes(self):
        self.assertEqual(check_ipa.problems(ENGINE, 40 << 20), [])

    def test_a_framework_binary_passes(self):
        self.assertEqual(
            check_ipa.problems(
                ENGINE
                + [
                    "Payload/Horizon.app/Frameworks/",
                    "Payload/Horizon.app/Frameworks/libhorizon.framework/",
                    "Payload/Horizon.app/Frameworks/libhorizon.framework/libhorizon",
                ],
                40 << 20,
            ),
            [],
        )

    def test_game_content_is_rejected(self):
        for extra in [
            "Payload/Horizon.app/assets/minecraft/textures/block/stone.png",
            "Payload/Horizon.app/assets/horizon-resources.json",
            "Payload/Horizon.app/resources/horizon-resources.json",
            "Payload/Horizon.app/Horizon.jar",
            "Payload/Horizon.app/sound.ogg",
            "Payload/minecraft/client.jar",
            "Payload/Horizon.app/data/minecraft/loot_table/x.json",
            "Payload/Horizon.app/level.dat",
            "Payload/extra.txt",
            "SomethingElse/Horizon.app/Horizon",
        ]:
            self.assertTrue(check_ipa.problems(ENGINE + [extra], 40 << 20), extra)

    def test_a_huge_ipa_is_rejected(self):
        self.assertTrue(check_ipa.problems(ENGINE, 400 << 20))


if __name__ == "__main__":
    unittest.main()
