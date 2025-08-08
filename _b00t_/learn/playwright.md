https://github.com/microsoft/playwright-mcp

# Capture Browser Logs with Playwright MCP in Cursor to Generate Reports
Source: https://egghead.io/capture-browser-logs-with-playwright-mcp-in-cursor-to-generate-reports~6vcr2

Debugging complex web forms can be a grind. You manually input data, check console logs, and painstakingly try to reproduce bugs. This lesson unveils a powerful, largely automated workflow using Cursor's AI agent in conjunction with the Playwright MCP to thoroughly test a web form, identify issues, and even fix the underlying code.

Key benefits:

Automated Test Execution: Leverage Playwright through Cursor to automatically interact with your live form.
AI-Driven Test Case Generation: The AI devises different input strategies to uncover bugs.
Log-Based Bug Reporting: Automatically capture and analyze console output to pinpoint issues.
Hands-Off Debugging: Delegate the testing and initial bug reporting to the AI.
Efficient Bug Fixing Cycle: Use AI-generated reports to quickly guide AI-assisted code corrections.
Focus on High-Level Direction: Instead of manual testing, you guide the AI's testing strategy and bug-fixing process.

Phase 1: Instrumenting Code with Exhaustive Logging

Context:

{@filePath} (e.g., App.tsx) is in Cursor's context.
Prompt: "In {@filePath}, add exhaustive console logging for all form events, state changes, and validation logic. Focus on capturing input values, current errors, and the outcome of validation checks. The goal is to have detailed logs for later analysis. Do not fix any existing code errors; only add logging. Stop after modifying the file."

Phase 2: AI-Driven Form Testing with Playwright and Log Analysis

Context:

{@formURL} (e.g., http://localhost:5173) is in Cursor's context.
The live form at {@formURL} should be running the version of the code that includes the exhaustive logging added in Phase 1.
Playwright MCP is available.
Prompt: "Using the Playwright MCP and the live form at {@formURL}:

Devise [N, e.g., 3-5] distinct test scenarios to fill out and submit the form. Include scenarios designed to trigger validation errors and edge cases (e.g., empty submission, invalid data types, boundary values, mismatched passwords).
For each scenario:
Execute the form interaction using Playwright.
Retrieve all browser console logs generated during that interaction.
Compile a consolidated 'Form Behavior Report' detailing:
Each test scenario and the inputs used.
Observed behavior/UI state (e.g., error messages displayed).
Key console log outputs relevant to errors or unexpected behavior.
Any discrepancies or potential bugs identified.
Phase 3: Fixing Bugs Based on AI Report and Re-Verification

Context:

The 'Form Behavior Report' from Phase 2 is in the chat history.
{@filePath} (e.g., App.tsx – the original file, not the one Playwright interacted with, unless it's the same deploy) is in Cursor's context.
The live form at {@formURL} can be updated/re-deployed with fixes.
Playwright MCP is available.
Prompt: "Based on the 'Form Behavior Report' (from our previous conversation) and the code in {@filePath}:

Analyze the reported issues and implement the necessary code changes in {@filePath} to fix them.
After applying the fixes and assuming the live form at {@formURL} is updated with these changes:
Re-run all previously defined test scenarios using the Playwright MCP.
Retrieve and analyze the console logs for each.
Provide a verification summary: confirm if the original bugs are fixed and if any new regressions were introduced."
Share with a coworker

Transcript
[00:00] Something in this form feels a little bit off, so I'm going to bring the file with the form into context of my agent. I'm going to ask it, please ignore all of the errors and instead focus on adding exhaustive console logging around all the events in the form, especially around validation. The goal here is we'll have enough information from the console logs to generate a report on what might be going wrong. Once that's done, please just stop. So we'll let this go through the file, generate a whole bunch of console logs around the events.

[00:28] Now with all of the logging in place we'll just accept this, start a new conversation, then grab the URL, clear out the reference to our file, paste in the URL, and ask please use the Playwright MCP to open this URL. And your goal is to fill out this form in a bunch of different ways attempting to break it. I've set up a lot of console logins so that each time after you fill out the form please check on the logs and generate reports on the behaviors that you discover. So please look at the form, think of three different ways to fill it out, fill it out all of those ways, read the console logs, and generate a report, I'm going to go get a sandwich. Then we'll just let this run.

[01:05] Remember this has no knowledge of this file. This file is not in context. So now it's going to use Playwright to open the browser. Let's scooch some things over for some more room. It looks like its first test case was to submit with a form empty, which is a great test.

[01:25] Now it's starting on test two and filling out various parts of the form. This is not me typing, this is just PlayWrite doing its own business while I'm just sitting back and doing nothing. And then it went and checked on the messages again. You can see that in this tool call up here, BrowserConsoleMessages. It grabs all of these and then writes up a report for that test case.

[01:45] And now all three test cases are completed and it wrote up reports for each of our test cases for us to investigate and get things fixed. Now hopefully the obvious next step is to grab this file back into context and just ask it to, Based on all these reports, please fix up our form. Then run your tests again to verify that your changes are fixed. Then again, you just sit back and relax and let it do all of the work.





