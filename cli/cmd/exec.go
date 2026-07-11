package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"
	"time"

	pb "github.com/shadowlynx/prox-cli/pkg/grpc/orchestrator"
	grpcclient "github.com/shadowlynx/prox-cli/pkg/grpc"
	"github.com/spf13/cobra"
)

var execCmd = &cobra.Command{
	Use:   "exec",
	Short: "Execute a single command and exit",
	Long: `Executes a single prompt and returns the result. 
Useful for scripting and piping.

Examples:
  slpx exec --prompt "scan target.com for open ports"
  echo "generate a reverse shell in Python" | slpx exec --prompt -
  slpx exec --prompt "audit token.sol" --type audit --target token.sol`,
	Run: runExec,
}

var (
	prompt     string
	execType   string
	target     string
	outputJSON bool
)

func init() {
	rootCmd.AddCommand(execCmd)
	execCmd.Flags().StringVarP(&prompt, "prompt", "p", "", "The prompt to execute")
	execCmd.Flags().StringVarP(&execType, "type", "t", "chat", "Execution type: chat, scan, exploit, build, audit, analyze, generate")
	execCmd.Flags().StringVar(&target, "target", "", "Target (URL, file path, IP)")
	execCmd.Flags().BoolVar(&outputJSON, "json", false, "Output in JSON format")
}

var execTypeMap = map[string]pb.ExecutionType{
	"chat":     pb.ExecutionType_EXECUTION_TYPE_CHAT,
	"scan":     pb.ExecutionType_EXECUTION_TYPE_SCAN,
	"exploit":  pb.ExecutionType_EXECUTION_TYPE_EXPLOIT,
	"build":    pb.ExecutionType_EXECUTION_TYPE_BUILD,
	"audit":    pb.ExecutionType_EXECUTION_TYPE_AUDIT,
	"analyze":  pb.ExecutionType_EXECUTION_TYPE_ANALYZE,
	"generate": pb.ExecutionType_EXECUTION_TYPE_GENERATE,
}

func runExec(cmd *cobra.Command, args []string) {
	// Get prompt from flag or stdin
	var input string
	if prompt == "-" || prompt == "" {
		stat, _ := os.Stdin.Stat()
		if (stat.Mode() & os.ModeCharDevice) == 0 {
			data, err := io.ReadAll(os.Stdin)
			if err != nil {
				exitWithError(fmt.Errorf("reading stdin: %w", err))
			}
			input = strings.TrimSpace(string(data))
		}
	}
	if prompt != "" && prompt != "-" {
		input = prompt
	}
	if input == "" {
		exitWithError(fmt.Errorf("no prompt provided. Use --prompt or pipe input"))
	}

	// Try to connect to orchestrator
	client, err := grpcclient.NewClient(grpcclient.ClientConfig{
		Address: orchestratorAddr,
	})
	if err != nil {
		exitWithError(fmt.Errorf("cannot connect to orchestrator at %s: %w\nMake sure the orchestrator is running:\n  cd orchestrator && cargo run", orchestratorAddr, err))
	}
	defer client.Close()

	// Determine execution type
	et, ok := execTypeMap[strings.ToLower(execType)]
	if !ok {
		et = pb.ExecutionType_EXECUTION_TYPE_CHAT
	}

	// Build request
	req := &pb.ExecuteRequest{
		Prompt:        input,
		ExecutionType: et,
		Target:        target,
	}

	// Execute
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	resp, err := client.Execute(ctx, req)
	if err != nil {
		exitWithError(fmt.Errorf("execution failed: %w", err))
	}

	if resp.Error != "" {
		fmt.Fprintf(os.Stderr, "Error: %s\n", resp.Error)
		if resp.Result != "" {
			fmt.Println(resp.Result)
		}
		os.Exit(1)
	}

	// Output
	if outputJSON {
		jsonData, _ := json.MarshalIndent(resp, "", "  ")
		fmt.Println(string(jsonData))
	} else {
		fmt.Println(resp.Result)
	}
}
