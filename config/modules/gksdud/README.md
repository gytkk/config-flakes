# gksdud

`modules.gksdud.enable` installs [gksdud](https://github.com/codingnoye/gksdud) on macOS. The common base enables it on Darwin, including both `devsisters-macbook` and `devsisters-macstudio`. The module has no effect on Linux, even if the option is enabled explicitly.

The [app package](../../packages/apps/gksdud/package.nix) pins the universal release ZIP by version and hash and preserves its code signature. It requires macOS 13 or later. Home Manager links the store bundle at `~/Applications/gksdud.app`. This works with the repository's [disabled app copying](../../base/compat/README.md).

Home Manager sets Right Command as the input-switch key, F19 as the internal target, and enables switching on key-down. gksdud owns the HID mappings and the macOS input-source shortcut. Activation writes only these four app preferences, preserving the app's keyboard records and restoration data. Editing them in the app lasts until the next Home Manager activation; restart gksdud after reapplying settings to a running instance.

Apply the configuration for the current Mac, for example:

```sh
home-manager switch --flake .#devsisters-macbook
```

On first use, open `~/Applications/gksdud.app` and allow gksdud in System Settings > Privacy & Security > Accessibility so key-down switching works. The upstream app is self-signed; if macOS blocks its first launch, allow it in Privacy & Security. These permissions remain user-managed. Use the app's own login-item option to start it at login; Home Manager does not manage automatic startup.

The old `hidutil-keyboard-mapping` launch agent is no longer managed. Removing its plist does not clear mappings already applied in the current session. Restart macOS after the first switch to clear those mappings, then open gksdud if its login-item option is off. Test Right Command switching between Korean and English after granting Accessibility permission.

Caps Lock-to-Control is configured separately in System Settings > Keyboard > Keyboard Shortcuts > Modifier Keys. Select each keyboard and assign Control to Caps Lock. Nix and gksdud do not manage this mapping.

The app checks for updates itself. Update the version and hash in the Nix package and reapply Home Manager instead of using the app's installer, which cannot update the immutable store bundle.
