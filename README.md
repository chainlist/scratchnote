# sv

Everything you need to build a Svelte project, powered by [`sv`](https://github.com/sveltejs/cli).

## Creating a project

If you're seeing this, you've probably already done this step. Congrats!

```sh
# create a new project
npx sv create my-app
```

To recreate this project with the same configuration:

```sh
# recreate this project
pnpm dlx sv@0.17.1 create --template minimal --types ts --add prettier eslint tailwindcss="plugins:none" --no-download-check --install pnpm scratchnote
```

## Windows prerequisites

The Rust side depends on `llama-cpp-2`, which compiles llama.cpp from source, for the
embedding model. It is built for the CPU only. On Windows this needs three things beyond
a normal Tauri setup. Missing any one of them fails the
build, and the error messages are not always obvious about the real cause.

**1. LLVM (for libclang)**

`llama-cpp-sys-2` generates its bindings with bindgen, which loads `libclang.dll` at build
time. Visual Studio does not ship it by default.

```sh
winget install --id LLVM.LLVM --exact
```

Then set `LIBCLANG_PATH` to the install's `bin` directory, e.g. `C:\Program Files\LLVM\bin`.
Without it the build fails with `Unable to find libclang`.

**2. Ninja on PATH**

The crate's build script passes `-G Ninja` to CMake unconditionally. Ninja ships with
Visual Studio but is not on `PATH` outside a Developer prompt. Either build from a
Developer PowerShell, or add the bundled copy to `PATH`:

```
<VS install>\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja
```

Without it CMake fails with `CMAKE_MAKE_PROGRAM is not set`.

**3. A short build path (MAX_PATH)**

This is the least obvious one. The nested `vulkan-shaders-gen` sub-build of the GPU build
the app had before produced paths over 260 characters when the repo lives somewhere deep like
`C:\Users\<name>\Documents\GitHub\scratchnote`. MSVC's PDB writer still honours the old
`MAX_PATH` limit even when Windows `LongPathsEnabled` is set to `1`, so the build fails
with a misleading `fatal error C1041: cannot open program database ... please use /FS`
(the `/FS` advice is a red herring, the flag is already passed).

Fix it by building somewhere short, e.g. set `CARGO_TARGET_DIR=C:\ct`, or clone the repo
to a shorter path. The CPU build has not been tried at a long path yet.

> If a CMake configure step fails partway through, delete
> `<target>/debug/build/llama-cpp-sys-2-*/out/build` before retrying. A half written
> `CMakeCache.txt` makes CMake skip reconfiguration and fail again with a stale error.

The installers bundle the Visual C++ runtime DLLs, which `build.rs` copies out of
`C:\Windows\System32`. The build machine therefore needs the Visual C++ redistributable
installed there, and the build fails with the missing file's name otherwise.

## macOS and Linux prerequisites

Neither needs a short build path. Ninja is still required, because
`src-tauri/.cargo/config.toml` selects it on every OS.

**macOS** (Apple Silicon):

```sh
xcode-select --install
brew install cmake ninja
```

**Linux**, on Ubuntu 24.04:

```sh
sudo apt install build-essential cmake ninja-build clang libclang-dev file \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
```

The packages only run
on systems whose glibc is at least as new as the build machine's (2.39 on Ubuntu 24.04),
so building on an older distro reaches more of them.

## Developing

Once you've created a project and installed dependencies with `npm install` (or `pnpm install` or `yarn`), start a development server:

```sh
npm run dev

# or start the server and open the app in a new browser tab
npm run dev -- --open
```

## Building

To create a production version of your app:

```sh
npm run build
```

You can preview the production build with `npm run preview`.

> To deploy your app, you may need to install an [adapter](https://svelte.dev/docs/kit/adapters) for your target environment.
