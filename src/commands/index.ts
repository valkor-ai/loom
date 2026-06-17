import { Command, Option } from "commander";
import {
  createArchitectureAcceptHandler,
  createArchitectureRequestHandler,
} from "./architecture";
import {
  createBrainstormAcceptHandler,
  createBrainstormAnswerHandler,
  createBrainstormConfirmHandler,
  createBrainstormStartHandler,
  createBrainstormStatusHandler,
} from "./brainstorm";
import { handleContinue } from "./continue";
import {
  handleDeployBootstrap,
  handleDeployDown,
  handleDeployInspect,
  handleDeployLogs,
  handleDeployPrepare,
  handleDeployRepair,
  handleDeployRun,
  handleDeployStatus,
  handleDeployUp,
  handleDeployValidate,
} from "./deploy";
import { handleInit } from "./init";
import { createInspectHandler } from "./inspect";
import {
  createKnowledgeAddHandler,
  createKnowledgeBuildHandler,
  createKnowledgeDisableHandler,
  createKnowledgeDiscardHandler,
  createKnowledgeEnableHandler,
  createKnowledgePendingHandler,
  createKnowledgeRemoveHandler,
  createKnowledgeStatusHandler,
  createKnowledgeUpdateHandler,
  handleKnowledgeList,
} from "./knowledge";
import { handleTokenSavingMetrics } from "./metrics";
import { createNextTaskHandler } from "./next-task";
import { createPlanHandler } from "./plan";
import { createPlanningContractCreateHandler } from "./planning-contract";
import { createRecordResultHandler } from "./record-result";
import { createRepairRequestHandler, createRepairSubmitHandler } from "./repair";
import {
  createRepositoryContextAcceptHandler,
  createRepositoryContextRequestHandler,
} from "./repository-context";
import {
  createReviewAcceptHandler,
  createReviewHandler,
  createReviewResolveHandler,
} from "./review";
import { runCommand } from "./run-command";
import { handleStatus } from "./status";
import {
  createTaskPlanAcceptHandler,
  createTaskPlanRequestHandler,
} from "./task-plan";
import {
  createTechnicalBaselineAcceptHandler,
  createTechnicalBaselineRequestHandler,
  handleTechnicalBaselineDetect,
} from "./technical-baseline";
import { LOOM_VERSION } from "../version";

export async function runCli(argv: string[]): Promise<void> {
  const program = new Command();

  program
    .name("loom")
    .description("Delivery layer for coding agents")
    .version(LOOM_VERSION);

  registerSimpleCommand(program, "init", "Initialize loom state", handleInit);
  registerSimpleCommand(program, "status", "Show loom project status", handleStatus);

  const metrics = program.command("metrics").description("Show local loom metrics");
  registerSimpleCommand(metrics, "token-saving", "Show token-saving telemetry", handleTokenSavingMetrics);

  registerKnowledgeCommands(program);

  program
    .command("inspect")
    .description("Read complete field value(s) from a loom request through its manifest refs")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--request <path>", "Request artifact path")
    .option("--field <field>", "Comma-separated field path(s) to read, for example task,outputContract.schemaShape")
    .option("--values-only", "Return only inspected field values in compact agent-facing output")
    .action(async (options) => {
      await runCommand("inspect", options, createInspectHandler({
        request: options.request,
        field: options.field,
        valuesOnly: options.valuesOnly,
      }));
    });

  program
    .command("plan [request...]")
    .description("Create a loom plan")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--request <text>", "Request text", collect, [])
    .option("--request-file <path>", "Request file path, or - for stdin", collect, [])
    .option("--requirement-file <path>", "Requirement document file path", collect, [])
    .option("--stdin", "Read request text from stdin")
    .option("--context <text>", "Supporting context text", collect, [])
    .option("--context-file <path>", "Supporting context file path", collect, [])
    .option("--skip-keyword-hints", "Skip advisory local TF-IDF keyword hints")
    .action(async (request: string[], options) => {
      await runCommand(
        "plan",
        options,
        createPlanHandler({
          positional: request,
          request: options.request,
          requestFile: options.requestFile,
          requirementFile: options.requirementFile,
          stdin: options.stdin,
          context: options.context,
          contextFile: options.contextFile,
          skipKeywordHints: options.skipKeywordHints,
        }),
      );
    });

  program
    .command("next-task")
    .description("Show next loom task")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--task-plan-run-id <id>", "TaskPlanRun id")
    .action(async (options) => {
      await runCommand(
        "next-task",
        options,
        createNextTaskHandler({
          deliveryId: options.deliveryId,
          phaseId: options.phaseId,
          taskPlanRunId: options.taskPlanRunId,
        }),
      );
    });

  program
    .command("record-result")
    .description("Record task result and evidence")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--input-file <path>", "Task result input file")
    .action(async (options) => {
      await runCommand(
        "record-result",
        options,
        createRecordResultHandler({
          deliveryId: options.deliveryId,
          phaseId: options.phaseId,
          inputFile: options.inputFile,
        }),
      );
    });

  const review = program.command("review").description("Review current loom delivery");
  review
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .action(async (options) => {
      await runCommand("review", options, createReviewHandler({
        deliveryId: options.deliveryId,
        phaseId: options.phaseId,
      }));
    });
  review
    .command("accept")
    .description("Accept a ReviewResult candidate")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--result-file <path>", "ReviewResult JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand("review.accept", options, createReviewAcceptHandler({
        deliveryId: stringOption(mergedOptions.deliveryId),
        phaseId: stringOption(mergedOptions.phaseId),
        resultFile: stringOption(mergedOptions.resultFile),
      }));
    });
  review
    .command("resolve")
    .description("Record a manual review resolution")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--candidate-file <path>", "ManualReviewResolution candidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "review.resolve",
        options,
        createReviewResolveHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          candidateFile: stringOption(mergedOptions.candidateFile),
        }),
      );
    });
  registerSimpleCommand(program, "continue", "Continue current loom delivery", handleContinue);

  const brainstorm = program.command("brainstorm").description("Internal Brainstorm operations for agent adapters");
  brainstorm
    .command("start [request...]")
    .description("Start an internal Brainstorm run")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--request <text>", "Request text", collect, [])
    .option("--request-file <path>", "Request file path, or - for stdin", collect, [])
    .option("--requirement-file <path>", "Requirement document file path", collect, [])
    .option("--input-file <path>", "Requirement input file path", collect, [])
    .option("--stdin", "Read request text from stdin")
    .option("--context <text>", "Supporting context text", collect, [])
    .option("--context-file <path>", "Supporting context file path", collect, [])
    .option("--skip-keyword-hints", "Skip advisory local TF-IDF keyword hints")
    .action(async (request: string[], options) => {
      await runCommand(
        "brainstorm.start",
        options,
        createBrainstormStartHandler({
          positional: request,
          request: options.request,
          requestFile: options.requestFile,
          requirementFile: options.requirementFile,
          inputFile: options.inputFile,
          stdin: options.stdin,
          context: options.context,
          contextFile: options.contextFile,
          skipKeywordHints: options.skipKeywordHints,
        }),
      );
    });

  brainstorm
    .command("accept")
    .description("Accept an agent-managed BrainstormCandidate")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--run-id <id>", "Brainstorm run id")
    .option("--request-id <id>", "BrainstormSessionRequest id")
    .option("--candidate-file <path>", "BrainstormCandidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "brainstorm.accept",
        options,
        createBrainstormAcceptHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          runId: stringOption(mergedOptions.runId),
          requestId: stringOption(mergedOptions.requestId),
          candidateFile: stringOption(mergedOptions.candidateFile),
        }),
      );
    });

  brainstorm
    .command("answer")
    .description("Record a Brainstorm clarification answer")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--run-id <id>", "Brainstorm run id")
    .option("--question-id <id>", "Clarification question id")
    .option("--answer <text>", "User answer text")
    .option("--answer-file <path>", "JSON or text file containing the answer")
    .option("--selected-option <id>", "Selected suggested option id", collect, [])
    .action(async (options) => {
      await runCommand(
        "brainstorm.answer",
        options,
        createBrainstormAnswerHandler({
          deliveryId: options.deliveryId,
          phaseId: options.phaseId,
          runId: options.runId,
          questionId: options.questionId,
          answer: options.answer,
          answerFile: options.answerFile,
          selectedOption: options.selectedOption,
        }),
      );
    });

  brainstorm
    .command("confirm")
    .description("Confirm or revise an interpreted Brainstorm patch")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--run-id <id>", "Brainstorm run id")
    .option("--confirmation-id <id>", "Confirmation request id")
    .option("--decision <confirmed|revise>", "Confirmation decision")
    .option("--revision <text>", "Freeform revision text when decision is revise")
    .option("--confirmation-file <path>", "JSON file containing confirmationId, decision, and optional revisionText")
    .action(async (options) => {
      await runCommand(
        "brainstorm.confirm",
        options,
        createBrainstormConfirmHandler({
          deliveryId: options.deliveryId,
          phaseId: options.phaseId,
          runId: options.runId,
          confirmationId: options.confirmationId,
          decision: options.decision,
          revision: options.revision,
          confirmationFile: options.confirmationFile,
        }),
      );
    });

  brainstorm
    .command("status")
    .description("Show a Brainstorm run status")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--run-id <id>", "Brainstorm run id")
    .action(async (options) => {
      await runCommand(
        "brainstorm.status",
        options,
        createBrainstormStatusHandler({ runId: options.runId }),
      );
    });

  const technicalBaseline = program.command("technical-baseline").description("Manage TechnicalBaseline contracts");
  registerSimpleCommand(technicalBaseline, "detect", "Detect repository signals", handleTechnicalBaselineDetect);
  technicalBaseline
    .command("request")
    .description("Create a TechnicalBaseline request for an agent")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--brainstorm-run-id <id>", "Brainstorm run id")
    .option("--project-kind <kind>", "greenfield|existing_project|unknown")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "technical-baseline.request",
        options,
        createTechnicalBaselineRequestHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          brainstormRunId: stringOption(mergedOptions.brainstormRunId),
          projectKind: stringOption(mergedOptions.projectKind),
        }),
      );
    });
  technicalBaseline
    .command("accept")
    .description("Accept a TechnicalBaseline candidate")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--candidate-file <path>", "TechnicalBaseline candidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "technical-baseline.accept",
        options,
        createTechnicalBaselineAcceptHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          candidateFile: stringOption(mergedOptions.candidateFile),
        }),
      );
    });

  const planningContract = program.command("planning-contract").description("Manage PlanningGenerationContract");
  planningContract
    .command("create")
    .description("Create a PlanningGenerationContract from confirmed Brainstorm scope")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--brainstorm-run-id <id>", "Brainstorm run id")
    .option("--phase-id <id>", "Roadmap phase id")
    .action(async (options) => {
      await runCommand(
        "planning-contract.create",
        options,
        createPlanningContractCreateHandler({
          deliveryId: options.deliveryId,
          brainstormRunId: options.brainstormRunId,
          phaseId: options.phaseId,
        }),
      );
    });

  const architecture = program.command("architecture").description("Manage ArchitectureArtifactContract");
  architecture
    .command("request")
    .description("Create an ArchitectureArtifactRequest for an agent")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--planning-contract-id <id>", "PlanningGenerationContract id")
    .option("--replace-active", "Replace the active architecture generation request for this phase")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "architecture.request",
        options,
        createArchitectureRequestHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          planningContractId: stringOption(mergedOptions.planningContractId),
          replaceActive: Boolean(mergedOptions.replaceActive),
        }),
      );
    });
  architecture
    .command("accept")
    .description("Accept Architecture section candidates")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--request-id <id>", "ArchitectureSectionsGenerationRequest id")
    .option("--repair-id <id>", "Architecture section repair id")
    .option("--candidate-file <path>", "Legacy ArchitectureArtifactContract candidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "architecture.accept",
        options,
        createArchitectureAcceptHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          candidateFile: stringOption(mergedOptions.candidateFile),
          requestId: stringOption(mergedOptions.requestId),
          repairId: stringOption(mergedOptions.repairId),
        }),
      );
    });

  const taskPlan = program.command("task-plan").description("Manage TaskPlan contracts");
  taskPlan
    .command("request")
    .description("Create a TaskPlanGenerationRequest for an agent")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--planning-contract-id <id>", "PlanningGenerationContract id")
    .option("--architecture-artifact-contract-id <id>", "ArchitectureArtifactContract id")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "task-plan.request",
        options,
        createTaskPlanRequestHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          planningContractId: stringOption(mergedOptions.planningContractId),
          architectureArtifactContractId: stringOption(mergedOptions.architectureArtifactContractId),
        }),
      );
    });
  taskPlan
    .command("accept")
    .description("Accept grouped TaskPlan candidates")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--request-id <id>", "TaskPlanGroupedGenerationRequest id")
    .option("--repair-id <id>", "TaskPlan repair id")
    .option("--candidate-file <path>", "Legacy TaskPlan candidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "task-plan.accept",
        options,
        createTaskPlanAcceptHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          candidateFile: stringOption(mergedOptions.candidateFile),
          requestId: stringOption(mergedOptions.requestId),
          repairId: stringOption(mergedOptions.repairId),
        }),
      );
    });

  const repositoryContext = program.command("repository-context").description("Manage RepositoryContext");
  repositoryContext
    .command("request")
    .description("Create a RepositoryContextRequest for an agent")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand("repository-context.request", options, createRepositoryContextRequestHandler({
        deliveryId: stringOption(mergedOptions.deliveryId),
        phaseId: stringOption(mergedOptions.phaseId),
      }));
    });
  repositoryContext
    .command("accept")
    .description("Accept a RepositoryContext candidate")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--request-id <id>", "RepositoryContextRequest id")
    .option("--candidate-file <path>", "RepositoryContext candidate JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand(
        "repository-context.accept",
        options,
        createRepositoryContextAcceptHandler({
          deliveryId: stringOption(mergedOptions.deliveryId),
          phaseId: stringOption(mergedOptions.phaseId),
          requestId: stringOption(mergedOptions.requestId),
          candidateFile: stringOption(mergedOptions.candidateFile),
        }),
      );
    });

  const repair = program.command("repair").description("Manage loom repair requests");
  repair
    .command("request")
    .description("Create a RepairRequest from the current route state")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--delivery-id <id>", "Delivery id")
    .option("--phase-id <id>", "Phase id")
    .option("--type <type>", "execution|task-result|taskplan|architecture")
    .option("--source <source>", "delivery|deploy")
    .option("--failure-ref <path>", "Deployment failure report path for deploy-sourced execution repair")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand("repair.request", options, createRepairRequestHandler({
        deliveryId: stringOption(mergedOptions.deliveryId),
        phaseId: stringOption(mergedOptions.phaseId),
        type: stringOption(mergedOptions.type),
        source: stringOption(mergedOptions.source),
        failureRef: stringOption(mergedOptions.failureRef),
      }));
    });
  repair
    .command("submit")
    .description("Submit a deploy-sourced synthetic repair result")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .option("--type <type>", "execution")
    .option("--source <source>", "deploy")
    .option("--repair-id <id>", "Repair id")
    .option("--result-file <path>", "DeployExecutionRepairTaskResult JSON file")
    .action(async (options, command) => {
      const mergedOptions = mergeParentOptions(options, command);
      await runCommand("repair.submit", options, createRepairSubmitHandler({
        type: stringOption(mergedOptions.type),
        source: stringOption(mergedOptions.source),
        repairId: stringOption(mergedOptions.repairId),
        resultFile: stringOption(mergedOptions.resultFile),
      }));
    });

  const deploy = program.command("deploy").description("Manage local loom deployment");
  registerDeployPrepareCommand(deploy, "run", "Prepare, build, start, validate, and report local deployment", handleDeployRun, "deploy.run");
  registerDeployPrepareCommand(deploy, "prepare", "Prepare local deployment", handleDeployPrepare, "deploy.prepare");
  registerDeployPrepareCommand(deploy, "up", "Start local deployment", handleDeployUp, "deploy.up");
  registerDeployCommand(deploy, "status", "Show local deployment status", handleDeployStatus, "deploy.status");
  registerDeployInspectCommand(deploy);
  registerDeployCommand(deploy, "validate", "Validate local deployment configuration and health", handleDeployValidate, "deploy.validate");
  registerDeployCommand(deploy, "logs", "Show local deployment logs", handleDeployLogs, "deploy.logs");
  registerDeployBootstrapCommand(deploy);
  registerDeployCommand(deploy, "down", "Stop local deployment", handleDeployDown, "deploy.down");
  registerDeployCommand(deploy, "repair", "Show local deployment repair request", handleDeployRepair, "deploy.repair");

  await program.parseAsync(argv, { from: "user" });
}

function registerSimpleCommand(
  program: Command,
  name: string,
  description: string,
  handler: Parameters<typeof runCommand>[2],
): void {
  program
    .command(name)
    .description(description)
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (options) => {
      await runCommand(name, options, handler);
    });
}

function registerKnowledgeCommands(program: Command): void {
  const knowledge = program.command("knowledge").description("Manage registered knowledge sources");

  knowledge
    .command("add [paths...]")
    .description("Queue a new knowledge source path set")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .requiredOption("--name <name>", "Globally unique knowledge source name")
    .action(async (paths: string[] | undefined, options) => {
      await runCommand("knowledge.add", options, createKnowledgeAddHandler({
        name: stringOption(options.name),
        paths: paths ?? [],
      }));
    });

  knowledge
    .command("update <name>")
    .description("Queue path changes for a knowledge source")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--add-path <path>", "Add a file or directory path", collect, [])
    .option("--remove-path <path>", "Remove a previously registered path", collect, [])
    .option("--replace-paths <paths...>", "Replace all registered paths")
    .action(async (name: string, options) => {
      await runCommand("knowledge.update", options, createKnowledgeUpdateHandler({
        name,
        addPath: stringArrayOption(options.addPath),
        removePath: stringArrayOption(options.removePath),
        replacePaths: stringArrayOption(options.replacePaths),
      }));
    });

  knowledge
    .command("pending [name]")
    .description("Show queued knowledge source changes")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string | undefined, options) => {
      await runCommand("knowledge.pending", options, createKnowledgePendingHandler({ name }));
    });

  knowledge
    .command("discard <name>")
    .description("Discard queued knowledge source changes")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.discard", options, createKnowledgeDiscardHandler({ name }));
    });

  knowledge
    .command("build <name>")
    .description("Build the mechanical knowledge index for a source")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.build", options, createKnowledgeBuildHandler({ name }));
    });

  registerKnowledgeSimpleCommand(knowledge, "list", "List knowledge sources", handleKnowledgeList, "knowledge.list");

  knowledge
    .command("status <name>")
    .description("Show knowledge source status")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.status", options, createKnowledgeStatusHandler({ name }));
    });

  knowledge
    .command("remove <name>")
    .description("Remove a knowledge source registration and pending queue")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.remove", options, createKnowledgeRemoveHandler({ name }));
    });

  knowledge
    .command("enable <name>")
    .description("Enable a built knowledge source")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.enable", options, createKnowledgeEnableHandler({ name }));
    });

  knowledge
    .command("disable <name>")
    .description("Disable a built knowledge source")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (name: string, options) => {
      await runCommand("knowledge.disable", options, createKnowledgeDisableHandler({ name }));
    });
}

function registerDeployCommand(
  program: Command,
  name: string,
  description: string,
  handler: Parameters<typeof runCommand>[2],
  commandId: string,
): void {
  program
    .command(name)
    .description(description)
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (options) => {
      await runCommand(commandId, options, handler);
    });
}

function registerKnowledgeSimpleCommand(
  program: Command,
  name: string,
  description: string,
  handler: Parameters<typeof runCommand>[2],
  commandId: string,
): void {
  program
    .command(name)
    .description(description)
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (options) => {
      await runCommand(commandId, options, handler);
    });
}

function registerDeployPrepareCommand(
  program: Command,
  name: string,
  description: string,
  handler: Parameters<typeof runCommand>[2],
  commandId: string,
): void {
  program
    .command(name)
    .description(description)
    .addOption(projectRootOption())
    .addOption(appPathOption())
    .addOption(healthcheckPathOption())
    .addOption(healthcheckCandidateOption())
    .addOption(healthcheckDisabledOption())
    .addOption(healthcheckAttemptsOption())
    .addOption(healthcheckIntervalOption())
    .addOption(healthcheckTimeoutOption())
    .addOption(healthcheckExpectedStatusOption())
    .addOption(providerOption())
    .addOption(forceGenerateOption())
    .addOption(reuseExistingOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (options) => {
      await runCommand(commandId, options, handler);
    });
}

function registerDeployInspectCommand(program: Command): void {
  program
    .command("inspect")
    .description("Inspect prepared deployment spec, runtime state, and latest repair request")
    .addOption(projectRootOption())
    .addOption(refreshOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .action(async (options) => {
      await runCommand("deploy.inspect", options, handleDeployInspect);
    });
}

function registerDeployBootstrapCommand(program: Command): void {
  program
    .command("bootstrap")
    .description("Show or explicitly run detected deployment bootstrap commands")
    .addOption(projectRootOption())
    .addOption(jsonOption())
    .addOption(compactOption())
    .option("--kind <kind>", "Bootstrap kind to run: prisma, django, rails, laravel, flyway, or liquibase")
    .option("--confirm", "Execute detected bootstrap commands inside the running app service")
    .action(async (options) => {
      await runCommand("deploy.bootstrap", options, handleDeployBootstrap);
    });
}

function projectRootOption(): Option {
  return new Option("--project-root <path>", "Project root");
}

function appPathOption(): Option {
  return new Option("--app-path <path>", "Deploy app path relative to project root");
}

function healthcheckPathOption(): Option {
  return new Option("--healthcheck-path <path>", "Override HTTP healthcheck path");
}

function healthcheckCandidateOption(): Option {
  return new Option("--healthcheck-candidate <path>", "Add/replace healthcheck candidate path").argParser(collect).default([]);
}

function healthcheckDisabledOption(): Option {
  return new Option("--healthcheck-disabled", "Disable HTTP healthcheck probing").default(false);
}

function healthcheckAttemptsOption(): Option {
  return new Option("--healthcheck-attempts <count>", "Healthcheck retry attempts");
}

function healthcheckIntervalOption(): Option {
  return new Option("--healthcheck-interval-ms <ms>", "Healthcheck retry interval in milliseconds");
}

function healthcheckTimeoutOption(): Option {
  return new Option("--healthcheck-timeout-ms <ms>", "Per-request healthcheck timeout in milliseconds");
}

function healthcheckExpectedStatusOption(): Option {
  return new Option("--healthcheck-expected-status-max <code>", "Highest HTTP status code considered healthy");
}

function providerOption(): Option {
  return new Option("--provider <provider>", "Provider policy: compose-existing, dockerfile-existing, or dockerfile-template");
}

function forceGenerateOption(): Option {
  return new Option("--force-generate", "Force generated Dockerfile/Compose assets").default(false);
}

function reuseExistingOption(): Option {
  return new Option("--reuse-existing <true|false>", "Whether existing Dockerfile/Compose assets may be reused");
}

function refreshOption(): Option {
  return new Option("--refresh", "Refresh running deployment status and health before inspecting").default(false);
}

function jsonOption(): Option {
  return new Option("--json", "Output JSON").default(true);
}

function compactOption(): Option {
  return new Option("--compact", "Output a compact agent-facing JSON envelope").default(false);
}

type CommandOptionBag = Record<string, string | boolean | string[] | undefined>;

function mergeParentOptions(options: CommandOptionBag, command: Command): CommandOptionBag {
  return {
    ...(typeof command.parent?.opts === "function" ? command.parent.opts<CommandOptionBag>() : {}),
    ...options,
  };
}

function stringOption(value: CommandOptionBag[string]): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function stringArrayOption(value: CommandOptionBag[string]): string[] | undefined {
  if (Array.isArray(value)) {
    return value.filter((entry): entry is string => typeof entry === "string");
  }
  if (typeof value === "string") {
    return [value];
  }
  return undefined;
}

function collect(value: string, previous: string[]): string[] {
  previous.push(value);
  return previous;
}
