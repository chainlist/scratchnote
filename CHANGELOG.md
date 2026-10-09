# Changelog

## [0.9.0](https://github.com/chainlist/scratchnote/compare/v0.8.0...v0.9.0) (2026-10-09)


### Features

* close what is open with Android's Back, and swipe on phones ([1e4d4d6](https://github.com/chainlist/scratchnote/commit/1e4d4d6186c977604dcde884d8ada4fc4734e355))
* switch space from the command center and open settings with Ctrl+, ([dffe727](https://github.com/chainlist/scratchnote/commit/dffe72736a55e639b86d169922186fdcec880daa))

## [0.8.0](https://github.com/chainlist/scratchnote/compare/v0.7.0...v0.8.0) (2026-10-09)


### Features

* fix sizes for phone screens ([1d50f24](https://github.com/chainlist/scratchnote/commit/1d50f243ac6454d2c418b8aa4f56a56cbb5d2fe5))
* keep the embedding model out of the notes folder ([9d78cc9](https://github.com/chainlist/scratchnote/commit/9d78cc9ebcaf24503027e3aed92db241dce5c49b))
* label each note with its part of life and job family ([f7e1c3b](https://github.com/chainlist/scratchnote/commit/f7e1c3b235dc7cab797191fa6ecb06f4b08660b3))
* run on Android ([1a6ed41](https://github.com/chainlist/scratchnote/commit/1a6ed417f4b7884b647e022f11e3c214ffcc9f5f))
* show what the embedder is doing in a dev build's status bar ([f7516e4](https://github.com/chainlist/scratchnote/commit/f7516e4f27092a7c435bb28f4c26b56dbce31d9a))


### Bug Fixes

* give toast action buttons the accent colour ([3476770](https://github.com/chainlist/scratchnote/commit/347677005d7e73834f3e96c7516c592723f7d5ee))
* grow icons and spacing with the phone's text size ([9c665f4](https://github.com/chainlist/scratchnote/commit/9c665f4cbda02e8808fd1e6bb7f9914783e859a9))

## [0.7.0](https://github.com/chainlist/scratchnote/compare/v0.6.0...v0.7.0) (2026-10-08)


### Features

* draw threads and names on the map, open the calendar from a day's date ([c0c52f5](https://github.com/chainlist/scratchnote/commit/c0c52f503e1bf8178fe16cf52ddd5b4d05e42b23))
* fit any window and text size, reach every view by keyboard, and say what failed ([3fd7007](https://github.com/chainlist/scratchnote/commit/3fd7007f0bb6a96a3bed41b02a10159207e76d82))
* read a day as a journal page, times in the margin and gaps to scale ([869e6cd](https://github.com/chainlist/scratchnote/commit/869e6cd3a29b1e84818f25c8ed37ffd94d5c4b03))
* show a thread's shape over time on its page ([121cf7c](https://github.com/chainlist/scratchnote/commit/121cf7c9487c857a402a170b3108771e6dd71d24))
* show the selected ribbon button in the accent colour ([3a2afd2](https://github.com/chainlist/scratchnote/commit/3a2afd27ad7815878d04f92a65e793a2ada47c7d))
* undo a change to threads, and name threads alike everywhere ([e2ece12](https://github.com/chainlist/scratchnote/commit/e2ece122ddd40928521f589593d6ed22735ac6a4))


### Performance Improvements

* draw only the tab shown and suggested threads 30 at a time ([1bdce29](https://github.com/chainlist/scratchnote/commit/1bdce29ad90f823eb5f1e930c9e42ed1ec23dc99))

## [0.6.0](https://github.com/chainlist/scratchnote/compare/v0.5.0...v0.6.0) (2026-10-05)


### Features

* a pinned name carries its threads one level below ([8749489](https://github.com/chainlist/scratchnote/commit/874948937fb9710f808783d4948af697d941aeec))
* find threads among the notes of each name mentioned ([505755c](https://github.com/chainlist/scratchnote/commit/505755cb522efb32f71cfa55bb3cbc90ff9d5822))
* give mentions and threads a pulse and a timeline ([1033759](https://github.com/chainlist/scratchnote/commit/103375910b6f8f1b03cfd7dec290906c25438c92))
* go back to today from a button on the left edge ([bd035a1](https://github.com/chainlist/scratchnote/commit/bd035a1932f57d45a38b006c3da4b8be89832a58))
* mentions become the app's own, read into search.db ([2b8657f](https://github.com/chainlist/scratchnote/commit/2b8657fef05d01fb721e9d0e9da47e53632fb1c9))
* offer a draft the name it looks like ([ea9c8ad](https://github.com/chainlist/scratchnote/commit/ea9c8ad5104344cc901894e28f1f2a6bf495eb06))
* pin threads and plugin pages to the left edge ([1765743](https://github.com/chainlist/scratchnote/commit/17657439090a0047a16dc07942ac44588aa525b8))
* say when a command keeps the window waiting ([28fc26c](https://github.com/chainlist/scratchnote/commit/28fc26c93725c283421ec2051552ba23c2f957fe))
* switch a name's page between its notes and its threads ([328252a](https://github.com/chainlist/scratchnote/commit/328252ac5d2d268fc0630396ead32a6bf5edff9e))
* the back arrow returns to the view it came from ([e59918d](https://github.com/chainlist/scratchnote/commit/e59918d336ee7b4c267418c8f6df1885854e3c01))


### Bug Fixes

* mark the left edge's button for the view shown, at a steady width ([9adb80a](https://github.com/chainlist/scratchnote/commit/9adb80a574aa916fab570490c468be022bf627a6))

## [0.5.0](https://github.com/chainlist/scratchnote/compare/v0.4.0...v0.5.0) (2026-10-04)


### ⚠ BREAKING CHANGES

* drop the chat model, keep only the embedding model

### Features

* divide a thread's notes by day ([521a98a](https://github.com/chainlist/scratchnote/commit/521a98ac6de57cefb3e3b99e9634fa4e1ba68b77))
* dock a thread beside the view and choose the order of its notes ([6600bf3](https://github.com/chainlist/scratchnote/commit/6600bf3497ed09bbd53c005e9504a88aa61e8b61))
* drop the chat model, keep only the embedding model ([11d9c4b](https://github.com/chainlist/scratchnote/commit/11d9c4b5fe30f98e953604930d508d8656c46b13))
* find notes on the map by their words and by their days ([83cd0e5](https://github.com/chainlist/scratchnote/commit/83cd0e56ddf85cada00bfad9a49457d0445ad099))
* group the map's notes into named categories and add a graph view ([3419adc](https://github.com/chainlist/scratchnote/commit/3419adc2ddf407480c29bd18795887a1c4b26deb))
* keep every note's place on a 2D map of the space in space.db ([2aef88a](https://github.com/chainlist/scratchnote/commit/2aef88afd6c606c588af928b2eac6b8adbfdeeaa))
* keep vectors, threads and thread edits in space.db ([50dfe94](https://github.com/chainlist/scratchnote/commit/50dfe9469751168e5eb6a120a7bbc4132b1b4fb9))
* never lay the map out again once it is laid out ([cafe357](https://github.com/chainlist/scratchnote/commit/cafe35761eae52e38ecb70f54ea47334a780e7d5))
* offer to remove the old chat model at launch ([505a9cc](https://github.com/chainlist/scratchnote/commit/505a9cc754756357af375c0b5592dc9c1524f291))
* show a spinner over a view that takes a moment to load ([bbe7d09](https://github.com/chainlist/scratchnote/commit/bbe7d09f39d245b3c76d7678232bcce7766aad20))
* show the map of the space by meaning on its own page ([951c27e](https://github.com/chainlist/scratchnote/commit/951c27e731e8ec42a28f648e8f56ef7bc2366ee9))
* suggest threads, and put notes in them by hand ([126d7a5](https://github.com/chainlist/scratchnote/commit/126d7a5236067d52a9e8016d3d50c8c07808e44d))


### Bug Fixes

* keep a new note's draft when the day is left ([b934244](https://github.com/chainlist/scratchnote/commit/b93424471d58d3c505b46c91e5f20bbda0f75ca8))
* keep the window responsive while the map is laid out ([1fbda7a](https://github.com/chainlist/scratchnote/commit/1fbda7ac9d6f2d8638dc26c4de349b61c6bffc2b))
* never make the window wait on placing notes in threads ([ee3c55d](https://github.com/chainlist/scratchnote/commit/ee3c55d5c898cd4516bc0217a5fb8a7d36a195d6))
* record the capture hotkey only when asked, not on focus ([b4df5b6](https://github.com/chainlist/scratchnote/commit/b4df5b6d6d4cf7706b8430f96c4b36bc27a5c4a4))


### Performance Improvements

* draw a note's menu once it is pointed at ([e05dbef](https://github.com/chainlist/scratchnote/commit/e05dbefe3041521ad9e6c87eab5bcb1779b8c5de))
* draw the map again only when the pointer reaches another note ([c88a2a4](https://github.com/chainlist/scratchnote/commit/c88a2a42bd396040eadf41350734369c8551753a))
* keep the map and its graph as they are when a note is saved ([37b8403](https://github.com/chainlist/scratchnote/commit/37b840324da2ded9dd6de54654585a4005ec2600))
* open a thread without titling every other ([c86f2ad](https://github.com/chainlist/scratchnote/commit/c86f2add7148f4854a7888a9777b20cda7e6bde1))
* settle the map's graph in a Web Worker ([40fc789](https://github.com/chainlist/scratchnote/commit/40fc789c3a5aa399b80b865720f5b30edf58f5b9))

## [0.4.0](https://github.com/chainlist/scratchnote/compare/v0.3.0...v0.4.0) (2026-10-01)


### Features

* bring a note back on the day it looks forward to ([f6c2250](https://github.com/chainlist/scratchnote/commit/f6c2250595823927f37aee1869cf243693f6a593))
* gather the notes about one thing into threads ([6ecf342](https://github.com/chainlist/scratchnote/commit/6ecf34279eaa1a89b2995c5ade98c41622c6c208))
* hide the chat button while the model is off ([6570be4](https://github.com/chainlist/scratchnote/commit/6570be49085a8839b882832ffc90cc019776fc53))
* keep the embedding model loaded while the chat model idles ([d127f3a](https://github.com/chainlist/scratchnote/commit/d127f3ab6b92ea6ee73fcb4b14648d3d2cc12a9f))
* keep the embedding model on while the model is off ([d8330f7](https://github.com/chainlist/scratchnote/commit/d8330f72be62eb9c7d1d6152f040ddc1ba2b531a))
* move a note or a page to another space ([35ef237](https://github.com/chainlist/scratchnote/commit/35ef237ffbffcdb8a3a9bc555439bf4960ef1ae6))
* name the old note a draft is about while it is written ([e260b7e](https://github.com/chainlist/scratchnote/commit/e260b7e8bae0a5b0def97620d599a54823357208))
* pick the space a quick capture goes into ([a87f569](https://github.com/chainlist/scratchnote/commit/a87f569f619ec95a621793fb89a56717cea2db0e))
* resizable dock, on the left or the right of the view ([33a2d7a](https://github.com/chainlist/scratchnote/commit/33a2d7a8170407b48dabead3c6a862cf7103d0a2))
* run the embedding model on the CPU ([c8a0f12](https://github.com/chainlist/scratchnote/commit/c8a0f122085450c66ccf4bddb7045b0859445f07))


### Bug Fixes

* stop a pending note glowing while the model cannot label it ([495a369](https://github.com/chainlist/scratchnote/commit/495a369d70a925ca7a3e56823eb60033901ad493))
* stop clipping the space name in the switcher ([120ccff](https://github.com/chainlist/scratchnote/commit/120ccff2ead85656d9f931a94c434dc45344db54))


### Performance Improvements

* count words once and cap search results ([fd9c7ab](https://github.com/chainlist/scratchnote/commit/fd9c7abba5107e18cf502ce08aa88c5c4ba32cf1))
* keep note text in search.db, not in memory ([ef30442](https://github.com/chainlist/scratchnote/commit/ef30442ea15bcadb54a6eb9bb7dd6d5cb9975206))
* read only the open space into memory ([78cc6e7](https://github.com/chainlist/scratchnote/commit/78cc6e70603c7a748f5da5951bdacb05fdc76330))
* read only the page files that changed ([e184c7d](https://github.com/chainlist/scratchnote/commit/e184c7d45758f6c86a770c91a40e43ce918bd552))
* stop repairing page stubs at launch ([652646f](https://github.com/chainlist/scratchnote/commit/652646fd46e28b0e9ee287104e28726d911cf709))

## [0.3.0](https://github.com/chainlist/scratchnote/compare/v0.2.0...v0.3.0) (2026-09-29)


### Features

* add a formatting toolbar to the editor ([d2da8b3](https://github.com/chainlist/scratchnote/commit/d2da8b342c4d5d12f26607b93da7ec8f36d4fcf7))
* add plugins, core and community ([6af1593](https://github.com/chainlist/scratchnote/commit/6af15932b000fc411c73b024c561da83c2ed02f1))
* check for updates in Settings &gt; About ([234d2f5](https://github.com/chainlist/scratchnote/commit/234d2f57bbc140e29b9f9747fd308ec64e15549d))
* journal view, mentions and stats core plugins, ribbon on the left ([f1b8760](https://github.com/chainlist/scratchnote/commit/f1b876026a403672d36714ed52d28403d438cd10))
* list the open tasks of a space ([f645c90](https://github.com/chainlist/scratchnote/commit/f645c90bb276050e46fc80a9b827643093df597e))
* save a note with the send button, as the capture window does ([9e56cc2](https://github.com/chainlist/scratchnote/commit/9e56cc2067699c430f9df2ac870e747cd461111f))
* tick tasks in notes and pages ([dba1e8f](https://github.com/chainlist/scratchnote/commit/dba1e8f4ebc794f87acd0e19de2e0362938ed3d9))


### Bug Fixes

* open a light window light ([296acd2](https://github.com/chainlist/scratchnote/commit/296acd2d9c7befe181439df03af08140f32c9201))
* stop the main window flashing "No model installed" as it opens ([a31b96b](https://github.com/chainlist/scratchnote/commit/a31b96bba29c374ebdb43d36f7a731cf1b3bb283))
* what's new in Settings &gt; About shows only the current version ([01377ca](https://github.com/chainlist/scratchnote/commit/01377cae6b2cf1f76665a3d12c6c9b073bd1f861))

## [0.2.0](https://github.com/chainlist/scratchnote/compare/v0.1.0...v0.2.0) (2026-09-28)


### Features

* add a send button to the capture window ([fa1456b](https://github.com/chainlist/scratchnote/commit/fa1456b9dd1229bd057597edab85cc5d5eec8a82))
* list every page of the space ([4de305c](https://github.com/chainlist/scratchnote/commit/4de305cf7ce5fe6ec199521eeae1fda99b283190))
* move and resize the capture window ([458a62e](https://github.com/chainlist/scratchnote/commit/458a62e05385823326a52d6f53726973a2c33b73))
* show what's new after an update, and an About tab in settings ([c622569](https://github.com/chainlist/scratchnote/commit/c622569fd5d345ca0c00751426eb913e3106baa1))

## 0.1.0 (2026-09-28)


### Features

* add app icon ([5eec1ee](https://github.com/chainlist/scratchnote/commit/5eec1ee1ddd510422500291e759694f768023442))
* added markdown edition and rendering ([44bbda7](https://github.com/chainlist/scratchnote/commit/44bbda743d704cba6e831a56303829c63e8c5d75))
* alphabet rail to jump through the tags page ([cef159d](https://github.com/chainlist/scratchnote/commit/cef159d687e7fca5c6e45c9cf97ea42851e74832))
* ask your notes, a streamed chat over the space's index ([4f6d6cc](https://github.com/chainlist/scratchnote/commit/4f6d6cc488dc541091c706348456269e009d7606))
* attach files and images to notes and pages ([3d58222](https://github.com/chainlist/scratchnote/commit/3d58222ad31214991912559e7702e5a2f7cf5816))
* calendar page, notes on any day, top bar title on scroll ([81b6d58](https://github.com/chainlist/scratchnote/commit/81b6d5809fac159bfd3b0e2d5cadd727b45a9685))
* capture and storage (milestone 1) ([8259673](https://github.com/chainlist/scratchnote/commit/82596737d82e7f84ea3356d06ef17a732aae6001))
* categories, appearance settings, custom window chrome ([e47e985](https://github.com/chainlist/scratchnote/commit/e47e985d17689be229b5332ee9ac1cb624ee7f3f))
* category sidebar, tag filter chips in search ([d6c05d9](https://github.com/chainlist/scratchnote/commit/d6c05d9716a0b1cdd2d22cb606a66a7cddfa1e35))
* chat prompt with a recent window and retrieved notes ([5392f31](https://github.com/chainlist/scratchnote/commit/5392f31144e93001e4f7a77f3240f348fb1a249c))
* delete a note from the day view ([d5c98c8](https://github.com/chainlist/scratchnote/commit/d5c98c894df0e37789ef46e9c1c755e6ef110fed))
* dock a page beside the day, neighbouring days, accent bold and italic ([72e141f](https://github.com/chainlist/scratchnote/commit/72e141fe27e2f9ed232ca0aea5273efb36bb160c))
* edit notes by hand and re-run enrichment ([8d91d94](https://github.com/chainlist/scratchnote/commit/8d91d9407362190762f44d966f2c643cb5855671))
* embedding model download in settings ([a1ed647](https://github.com/chainlist/scratchnote/commit/a1ed6473d33bf5f6c939ea6e924b36d8feb174cd))
* embedding trait and per-space vector store ([75ca071](https://github.com/chainlist/scratchnote/commit/75ca071cd15f660d9d272c1f412f769dc70e56e9))
* enrichment (milestone 3) ([0272982](https://github.com/chainlist/scratchnote/commit/027298270f9b407273171ffe920fd0d3f62a3f99))
* first-run onboarding, model benchmark, folder and hotkey pickers ([2be3c42](https://github.com/chainlist/scratchnote/commit/2be3c42be61a74401691e6b44564f06b73354629))
* GPU acceleration with a settings toggle ([da7a6a5](https://github.com/chainlist/scratchnote/commit/da7a6a554cbfe1632047442575abc46ae3312c34))
* index.jsonl, startup rebuild and the file watcher (milestone 2) ([444e4f1](https://github.com/chainlist/scratchnote/commit/444e4f12404588c1dd07bc7ee8b538d6831356c9))
* interface translations, font and model settings ([62de692](https://github.com/chainlist/scratchnote/commit/62de692a22785f8e34c9735661a5a7f3d8b05f4a))
* journal day view, always-on GPU build, clearer enrichment errors ([25d835b](https://github.com/chainlist/scratchnote/commit/25d835be886f0ae415bd005013731910d833bd2a))
* journal-focused layout with command center and floating chat ([7408b9f](https://github.com/chainlist/scratchnote/commit/7408b9f4b51ec2c47599babc131ff690d7b1fea8))
* keep each space's vectors in sync with its index ([84bde1a](https://github.com/chainlist/scratchnote/commit/84bde1a989eb2954cb4c7077eb27670f2d8d969c))
* label notes in the interface language, add notes from the day view ([af0a547](https://github.com/chainlist/scratchnote/commit/af0a547ed55295a8de7c6c8a17533d70b23a02fb))
* llama.cpp embedder for Qwen3-Embedding-0.6B ([208c296](https://github.com/chainlist/scratchnote/commit/208c2966596d6ac2fe05f65b186eeae28b738b8d))
* pages for longer writing ([657a69f](https://github.com/chainlist/scratchnote/commit/657a69f8114ece5203c8e7f02aa6e82120427e15))
* retrieve notes for the chat with the embedding model ([9e7cb9a](https://github.com/chainlist/scratchnote/commit/9e7cb9aba98211e835f67b63617ca844df2c7372))
* rework font + models handling ([b0a0e12](https://github.com/chainlist/scratchnote/commit/b0a0e1222c2c07d078dead5ebaf27c0aed1a5c5b))
* search by meaning in the command center ([df4acf4](https://github.com/chainlist/scratchnote/commit/df4acf46fb2c839b5e5143bea1330ebbc054d829))
* settings screen ([9121983](https://github.com/chainlist/scratchnote/commit/9121983c869c5f12910797df5505f2e2a2995673))
* similar notes from the embedding vectors ([ba56c8a](https://github.com/chainlist/scratchnote/commit/ba56c8a78aa7466c3bb084d3af385713ad88441f))
* single instance, Dock reopen and a --capture flag for Wayland ([5c5b6c5](https://github.com/chainlist/scratchnote/commit/5c5b6c5ac69b42ff09ce70d2a659957c03d34485))
* spaces, full note editor, model status tooltips ([2ffd037](https://github.com/chainlist/scratchnote/commit/2ffd037456eeff67e7908231502e63271d91e72d))
* switch models, use a custom GGUF and check for updates ([b088c9e](https://github.com/chainlist/scratchnote/commit/b088c9e2db592823898ffd7d351dab9fc7cc7852))
* tags and search (milestone 4) ([4931e5f](https://github.com/chainlist/scratchnote/commit/4931e5fd525ea16d48568b074e9a957de4a19a1e))
* unload the model after it sits idle (milestone 5) ([726f4cf](https://github.com/chainlist/scratchnote/commit/726f4cf0668b08fd92a3ddab0d790e10ed91c3c5))
* update the app from GitHub Releases ([c83f1a7](https://github.com/chainlist/scratchnote/commit/c83f1a76057f354e2487545eef49a40116d04e78))


### Bug Fixes

* installers carry the runtime libraries a fresh machine lacks ([8b59b5b](https://github.com/chainlist/scratchnote/commit/8b59b5bf4cb9397ab53decb5e245c2c26fa125c7))
* no sideways scrollbar while a note is enriching ([dc0a128](https://github.com/chainlist/scratchnote/commit/dc0a128a216135453a6da384fe63209ca8a7d794))
* overlaps, settings notices, chat lists and Esc, still glow with the model off ([7c445a7](https://github.com/chainlist/scratchnote/commit/7c445a779ad4437a400063721e1558a5bb21f0a5))
* point the model catalog at repos that actually exist ([2a37523](https://github.com/chainlist/scratchnote/commit/2a375234cd1fd5fadaa700f0040ce99f78507527))
* relabel notes whose body was edited in another editor ([c5c7675](https://github.com/chainlist/scratchnote/commit/c5c7675ba6b2c499e5dcf62f3165685c9bc0ac2d))
* save vectors every 500 notes so a first backfill survives a quit ([7aafe62](https://github.com/chainlist/scratchnote/commit/7aafe623154bdb4a3327882ecbf16c745fd713c9))
* similar notes in small spaces ([8a33cca](https://github.com/chainlist/scratchnote/commit/8a33cca5dacfe81be3554b2578663412bd480a9a))
* size the chat's index window by characters ([4f72429](https://github.com/chainlist/scratchnote/commit/4f724298c322d04065fc2ca1979b8b4bf05bc3ad))
* tag named things by name and stop padding with existing tags ([d4177e7](https://github.com/chainlist/scratchnote/commit/d4177e7e762fc2fa5ac0c6292ee059414282c71d))
* tag notes by subject matter, not by kind of note ([b40deee](https://github.com/chainlist/scratchnote/commit/b40deeecc8fe66376e36fe8334767b1996bbeaba))
