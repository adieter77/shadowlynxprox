package cmd

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/spf13/cobra"
)

var setupCmd = &cobra.Command{
	Use:   "setup",
	Short: "Interactive setup wizard",
	Long:  `Run the first-time setup wizard for Shadowlynx ProX.`,
	Run:   runSetupWizard,
}

func runSetupWizard(cmd *cobra.Command, args []string) {
	reader := bufio.NewReader(os.Stdin)

	fmt.Println()
	fmt.Println("  ╔══════════════════════════════════════════╗")
	fmt.Println("  ║     Shadowlynx ProX — Setup Wizard       ║")
	fmt.Println("  ╚══════════════════════════════════════════╝")
	fmt.Println()

	fmt.Println("━━━ Step 1: System Check ━━━")
	checks := map[string]string{
		"docker":   "docker --version",
		"redis":    "redis-cli --version",
		"go":       "go version",
		"rust":     "rustc --version",
		"python3":  "python3 --version",
	}
	allGood := true
	for name, testCmd := range checks {
		parts := strings.Split(testCmd, " ")
		c := exec.Command(parts[0], parts[1:]...)
		output, err := c.CombinedOutput()
		if err != nil {
			fmt.Printf("  ✗ %s — not found\n", name)
			allGood = false
		} else {
			version := strings.TrimSpace(string(output))
			if len(version) > 60 {
				version = version[:60] + "..."
			}
			fmt.Printf("  ✓ %s — %s\n", name, version)
		}
	}
	fmt.Println()
	if !allGood {
		fmt.Println("  Some dependencies are missing. Install them and run setup again.")
		fmt.Println()
	}

	fmt.Println("━━━ Step 2: AI Provider Configuration ━━━")
	fmt.Println()
	fmt.Println("  1. Ollama (free, local) — Recommended")
	fmt.Println("  2. Anthropic Claude (API key)")
	fmt.Println("  3. OpenAI GPT (API key)")
	fmt.Println("  4. DeepSeek (API key)")
	fmt.Println("  5. Skip")
	fmt.Println()

	fmt.Print("Select provider [1]: ")
	choice, _ := reader.ReadString('\n')
	choice = strings.TrimSpace(choice)
	if choice == "" { choice = "1" }

	configs := map[string]string{}
	switch choice {
	case "1":
		fmt.Println()
		fmt.Print("Model to use [llama3.1:8b]: ")
		model, _ := reader.ReadString('\n')
		model = strings.TrimSpace(model)
		if model == "" { model = "llama3.1:8b" }
		configs["DEFAULT_PROVIDER"] = "ollama"
		configs["OLLAMA_ENDPOINT"] = "http://localhost:11434"
		configs["OLLAMA_DEFAULT_MODEL"] = model
		fmt.Printf("  ✓ Ollama configured (model: %s)\n", model)
	case "2":
		fmt.Print("Anthropic API key: ")
		key, _ := reader.ReadString('\n')
		key = strings.TrimSpace(key)
		if key != "" {
			configs["ANTHROPIC_API_KEY"] = key
			configs["DEFAULT_PROVIDER"] = "anthropic"
			fmt.Println("  ✓ Anthropic configured")
		}
	case "3":
		fmt.Print("OpenAI API key: ")
		key, _ := reader.ReadString('\n')
		key = strings.TrimSpace(key)
		if key != "" {
			configs["OPENAI_API_KEY"] = key
			configs["DEFAULT_PROVIDER"] = "openai"
			fmt.Println("  ✓ OpenAI configured")
		}
	case "4":
		fmt.Print("DeepSeek API key: ")
		key, _ := reader.ReadString('\n')
		key = strings.TrimSpace(key)
		if key != "" {
			configs["DEEPSEEK_API_KEY"] = key
			configs["DEFAULT_PROVIDER"] = "deepseek"
			fmt.Println("  ✓ DeepSeek configured")
		}
	}
	fmt.Println()

	if len(configs) > 0 {
		envPath := "../ai-core/.env"
		fmt.Printf("Writing configuration to %s...\n", envPath)

		existing := map[string]string{}
		if data, err := os.ReadFile(envPath); err == nil {
			for _, line := range strings.Split(string(data), "\n") {
				line = strings.TrimSpace(line)
				if line == "" || strings.HasPrefix(line, "#") { continue }
				parts := strings.SplitN(line, "=", 2)
				if len(parts) == 2 { existing[parts[0]] = parts[1] }
			}
		}
		for k, v := range configs { existing[k] = v }

		var lines []string
		for k, v := range existing {
			lines = append(lines, fmt.Sprintf("%s=%s", k, v))
		}
		os.WriteFile(envPath, []byte(strings.Join(lines, "\n")+"\n"), 0644)
		fmt.Println("  ✓ Configuration saved")
	}

	fmt.Println()
	fmt.Println("══════════════════════════════════════════")
	fmt.Println("  Setup complete!")
	fmt.Println()
	fmt.Println("  Start the system:")
	fmt.Println("    1. cd orchestrator && cargo run        # Orchestrator (gRPC :50052)")
	fmt.Println("    2. cd ai-core && source .venv/bin/activate")
	fmt.Println("       python -m src.main                  # AI Core (gRPC :50051)")
	fmt.Println("    3. slpx chat                            # Start chatting")
	fmt.Println("    4. slpx plugin load <file.wasm>        # Load a plugin")
	fmt.Println()
	fmt.Println("  Or use the all-in-one script:")
	fmt.Println("    ./scripts/start-all.sh")
	fmt.Println("══════════════════════════════════════════")
	fmt.Println()
}

func init() {
	rootCmd.AddCommand(setupCmd)
}
