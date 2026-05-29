# GitHub Screenshots

This folder is for screenshots used on the repo page or release notes.

Keep screenshots clean. Do not include real chats, keys, account names, local file paths, or anything private.

To regenerate them:

```powershell
$env:AI_CHAT_SCREENSHOT_USERNAME = "your-username"
$env:AI_CHAT_SCREENSHOT_PASSWORD = "your-password"
powershell -ExecutionPolicy Bypass -File scripts\capture-github-screenshots.ps1
```

Expected files:

1. `01-setup.png`
2. `02-lock.png`
3. `03-chat-home.png`
4. `04-new-chat.png`
5. `05-chat-drawer.png`
6. `06-models.png`
7. `07-gallery.png`
8. `08-backups.png`
9. `09-diagnostics.png`
10. `10-settings.png`
11. `11-theme-paper.png`
12. `12-theme-alpine.png`
13. `13-theme-clay.png`
14. `14-theme-mono.png`
15. `15-generation.png`
16. `16-paths.png`
17. `17-security.png`
18. `18-chat-night.png`
19. `19-lock-screen.png`
20. `20-chat-final.png`
