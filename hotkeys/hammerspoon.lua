-- ~/.hammerspoon/init.lua
hs.hotkey.bind({ "ctrl", "alt", "cmd" }, "B", function()
  hs.task.new(os.getenv("HOME") .. "/.local/bin/brevity", nil):start()
end)

-- Optional: same chord plus shift puts the original text back.
hs.hotkey.bind({ "ctrl", "alt", "cmd", "shift" }, "B", function()
  hs.task.new(os.getenv("HOME") .. "/.local/bin/brevity", nil, { "--restore" }):start()
end)
