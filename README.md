# Waybar taskwarrior hook
Hook to integrate taskwarrior's tasks into a custom waybar module
## How to use

1. Clone the repo:
```
git clone git@github.com:spivi269/waybar-taskwarrior-hook.git
```

2. Look at [sample-waybar-config.jsonc](sample-waybar-config.jsonc) and adapt your own waybar's config.jsonc accordingly:
```
more sample-waybar-config.jsonc
```
_Notice that you will need the `"signal": 8`, as the hook uses SIGRTMIN+8 to signal a change to waybar_

3. Build the hook:
```
cd on-exit-hook-waybar/
cargo build --release
```

4. Move the program to your task's hooks directory:
```
mv target/release/on-exit-hook-waybar ~/.task/hooks/
```

5. Start your waybar with the [start-waybar.sh](start-waybar.sh) script or use any other way to write waybar's PID to ~/.cache/waybar.pid
