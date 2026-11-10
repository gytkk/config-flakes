{ config, ... }:

{
  modules.claudeSessionMining = {
    enable = true;
    runnerPath = "${config.home.homeDirectory}/workspace/gytkk-space/automation/mine-sessions.sh";
  };
}
