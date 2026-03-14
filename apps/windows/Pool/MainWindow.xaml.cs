using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using PoolCore.Models;
using PoolCore.Services;

namespace Pool;

/// <summary>
/// Interaction logic for MainWindow.xaml
/// </summary>
public partial class MainWindow : Window
{
    private readonly PoolService _poolService;

    public string CurrentViewTitle { get; set; } = "Projects";
    public string Version { get; set; } = "1.0.0";
    public ObservableCollection<Project> Projects { get; set; }

    public MainWindow(PoolService poolService)
    {
        InitializeComponent();
        _poolService = poolService;

        // Initialize sample data
        Projects = new ObservableCollection<Project>
        {
            new Project { Id = "1", Name = "Demo Project", ShotCount = 12 },
            new Project { Id = "2", Name = "Animation Short", ShotCount = 24 },
            new Project { Id = "3", Name = "Commercial 2024", ShotCount = 8 },
            new Project { Id = "4", Name = "Music Video", ShotCount = 16 }
        };

        // Get version from core
        Version = _poolService.GetVersion();

        DataContext = this;
    }

    #region Navigation

    private void OnProjectsClick(object sender, RoutedEventArgs e)
    {
        CurrentViewTitle = "Projects";
        ProjectsView.Visibility = Visibility.Visible;
        TimelineView.Visibility = Visibility.Collapsed;
        NodeEditorView.Visibility = Visibility.Collapsed;
        OnPropertyChanged(nameof(CurrentViewTitle));
    }

    private void OnShotsClick(object sender, RoutedEventArgs e)
    {
        CurrentViewTitle = "Shots";
        ProjectsView.Visibility = Visibility.Visible;
        TimelineView.Visibility = Visibility.Collapsed;
        NodeEditorView.Visibility = Visibility.Collapsed;
        OnPropertyChanged(nameof(CurrentViewTitle));
    }

    private void OnTimelineClick(object sender, RoutedEventArgs e)
    {
        CurrentViewTitle = "Timeline";
        ProjectsView.Visibility = Visibility.Collapsed;
        TimelineView.Visibility = Visibility.Visible;
        NodeEditorView.Visibility = Visibility.Collapsed;
        OnPropertyChanged(nameof(CurrentViewTitle));
    }

    private void OnNodeEditorClick(object sender, RoutedEventArgs e)
    {
        CurrentViewTitle = "Node Editor";
        ProjectsView.Visibility = Visibility.Collapsed;
        TimelineView.Visibility = Visibility.Collapsed;
        NodeEditorView.Visibility = Visibility.Visible;
        OnPropertyChanged(nameof(CurrentViewTitle));
    }

    private void OnWorkflowsClick(object sender, RoutedEventArgs e)
    {
        CurrentViewTitle = "Workflow Templates";
        ProjectsView.Visibility = Visibility.Visible;
        TimelineView.Visibility = Visibility.Collapsed;
        NodeEditorView.Visibility = Visibility.Collapsed;
        OnPropertyChanged(nameof(CurrentViewTitle));
    }

    #endregion

    #region Actions

    private void OnNewProjectClick(object sender, RoutedEventArgs e)
    {
        var dialog = new NewProjectDialog();
        if (dialog.ShowDialog() == true)
        {
            var project = _poolService.CreateProject(dialog.ProjectName);
            Projects.Add(new Project
            {
                Id = project.Id,
                Name = project.Name,
                ShotCount = 0
            });
        }
    }

    private void OnImportClick(object sender, RoutedEventArgs e)
    {
        var dialog = new Microsoft.Win32.OpenFileDialog
        {
            Title = "Import Files",
            Multiselect = true,
            Filter = "All Files (*.*)|*.*|Video Files (*.mp4;*.mov;*.avi)|*.mp4;*.mov;*.avi|Image Files (*.png;*.jpg;*.exr)|*.png;*.jpg;*.exr"
        };

        if (dialog.ShowDialog() == true)
        {
            foreach (var fileName in dialog.FileNames)
            {
                System.Diagnostics.Debug.WriteLine($"Importing: {fileName}");
            }
        }
    }

    private void OnProjectClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is Border border && border.DataContext is Project project)
        {
            CurrentViewTitle = $"Project: {project.Name}";
            OnPropertyChanged(nameof(CurrentViewTitle));
            // Navigate to project details
        }
    }

    #endregion

    #region Timeline Controls

    private void OnTimelineStart(object sender, RoutedEventArgs e)
    {
        System.Diagnostics.Debug.WriteLine("Timeline: Go to start");
    }

    private void OnTimelinePrev(object sender, RoutedEventArgs e)
    {
        System.Diagnostics.Debug.WriteLine("Timeline: Previous frame");
    }

    private void OnTimelinePlay(object sender, RoutedEventArgs e)
    {
        var button = sender as Button;
        if (button?.Content?.ToString() == "Play")
        {
            button.Content = "Pause";
            System.Diagnostics.Debug.WriteLine("Timeline: Play");
        }
        else if (button != null)
        {
            button.Content = "Play";
            System.Diagnostics.Debug.WriteLine("Timeline: Pause");
        }
    }

    private void OnTimelineNext(object sender, RoutedEventArgs e)
    {
        System.Diagnostics.Debug.WriteLine("Timeline: Next frame");
    }

    private void OnTimelineEnd(object sender, RoutedEventArgs e)
    {
        System.Diagnostics.Debug.WriteLine("Timeline: Go to end");
    }

    #endregion

    protected void OnPropertyChanged(string propertyName)
    {
        System.ComponentModel.PropertyChangedEventHandler? handler = null;
        if (handler != null)
        {
            handler(this, new System.ComponentModel.PropertyChangedEventArgs(propertyName));
        }
    }
}

/// <summary>
/// Simple dialog for creating new projects
/// </summary>
public class NewProjectDialog : Window
{
    private TextBox _nameTextBox = null!;

    public string ProjectName => _nameTextBox.Text;

    public NewProjectDialog()
    {
        Title = "New Project";
        Width = 400;
        Height = 200;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        ResizeMode = ResizeMode.NoResize;

        var grid = new Grid();
        grid.Margin = new Thickness(20);

        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        // Title
        var title = new TextBlock
        {
            Text = "Create New Project",
            FontSize = 18,
            FontWeight = FontWeights.SemiBold,
            Margin = new Thickness(0, 0, 0, 16)
        };
        Grid.SetRow(title, 0);
        grid.Children.Add(title);

        // Input
        var stackPanel = new StackPanel();
        Grid.SetRow(stackPanel, 1);

        var label = new TextBlock
        {
            Text = "Project Name",
            Margin = new Thickness(0, 0, 0, 8)
        };
        stackPanel.Children.Add(label);

        _nameTextBox = new TextBox
        {
            Padding = new Thickness(8),
            Text = "New Project"
        };
        stackPanel.Children.Add(_nameTextBox);

        grid.Children.Add(stackPanel);

        // Buttons
        var buttonPanel = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(0, 16, 0, 0)
        };
        Grid.SetRow(buttonPanel, 2);

        var cancelButton = new Button
        {
            Content = "Cancel",
            Width = 80,
            Margin = new Thickness(0, 0, 8, 0)
        };
        cancelButton.Click += (s, e) => DialogResult = false;
        buttonPanel.Children.Add(cancelButton);

        var createButton = new Button
        {
            Content = "Create",
            Width = 80
        };
        createButton.Click += (s, e) => DialogResult = true;
        buttonPanel.Children.Add(createButton);

        grid.Children.Add(buttonPanel);

        Content = grid;

        // Focus on text box
        Loaded += (s, e) => _nameTextBox.Focus();
        _nameTextBox.SelectAll();
    }
}
