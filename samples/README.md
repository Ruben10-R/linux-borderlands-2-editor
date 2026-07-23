# samples/

Test saves live here but are **not** committed — real Borderlands 2 saves embed
your Steam ID and character data, so `samples/*.sav` is gitignored.

To run the proof-of-concept, drop a save file in here named `save0001.sav`:

```bash
# Aspyr Linux port save location:
cp "$HOME/.local/share/aspyr-media/borderlands 2/willowgame/savedata/"*/save0001.sav \
   samples/save0001.sav

# then, from the repo root:
./run.sh
```

Always work against **copies** — never point the tools at your live savedata
directory until an in-game load has been proven.
