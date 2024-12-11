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

> [!NOTE]
>
> Notice that you will need the `"signal": 8`, as the hook uses SIGRTMIN+8 to signal a change to waybar

3. Build and install the hook:

If using the default location for task config (~/.task/hooks/):

```
make install
```

If you changed the default path to something else you can do:

```
make install TARGET_DIR=/your/custom/directory
```

The hook sends SIGRTMIN+8 to all waybar instances to update.
