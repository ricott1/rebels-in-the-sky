# Rebels in the Sky

![Splash screen](demo/splash.png)

It's the year 2101. Corporations have taken over the world.
The only way to be free is to join a pirate crew and start plundering the galaxy. The only mean of survival is to play basketball.

Now it's your turn to go out there and make a name for yourself. Create your crew and start wondering the galaxy in search of worthy basketball opponents.

## Download

Compiled binaries of the last release can be downloaded at https://rebels.frittura.org.

## Build

You need to have the rust toolchain installed --> https://www.rust-lang.org/tools/install. Then you can build the game with

`cargo build --release`

## Run

This game runs as a terminal application, meaning that you just need to run the executable from your terminal with

`./rebels`

If you downloaded the binaries, you will first need to give execution permissions to the executable with

`chmod +x rebels`

Suggested minimal terminal size: 162x48. Not all terminals support the game colors nicely, so you might need to try different ones. Here is a list of tested terminals:

-   Linux: whatever the default terminal is, it should work
-   MacOs: [iTerm2](https://iterm2.com/)
-   Windows: need a someone to test it :)

**Important**: currently the game generates local teams by defalt. This behaviour can be disabled by passing the `-f` flag to the executable. In the future, when more players will be available, the game will default to online teams only.

## Credits

-   Planet gifs were generated using the [pixel planet generator](https://deep-fold.itch.io/pixel-planet-generator) by [Deep Fold](https://deep-fold.itch.io/).
-   Music of the songs "Rebels in the sky", "Spacepirates", and "Rap the kasta" were generated with [Suno](https://app.suno.ai/).
-   Special thanks to [Il Deposito](https://www.ildeposito.org)

## Contribution

It is almost guaranteed that you will encounter bugs along your journey. If you do, please open an issue and describe what happened. If you are a developer and want to contribute, feel free to open a pull request.

## License

This software is released under the [GPLv3](https://www.gnu.org/licenses/gpl-3.0.en.html) license.
