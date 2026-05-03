const vscode = {
    commands: {
        executeCommand: async () => {}
    },
    window: {
        showInformationMessage: async () => {}
    },
    workspace: {
        getConfiguration: () => ({})
    },
    extensions: {
        getExtension: () => null
    }
};

export default vscode;