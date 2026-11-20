package dev.yalin.sample.ui.main

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation3.runtime.NavKey
import dev.yalin.sample.R
import dev.yalin.sample.data.Dataset
import dev.yalin.sample.data.DefaultDataRepository
import dev.yalin.sample.data.MEASURED_RUNS
import dev.yalin.sample.data.ParseResult
import dev.yalin.sample.data.ParserTime
import dev.yalin.sample.ui.theme.YalinSampleTheme

@Composable
fun MainScreen(
  onItemClick: (NavKey) -> Unit,
  modifier: Modifier = Modifier,
  viewModel: MainScreenViewModel = viewModel { MainScreenViewModel(DefaultDataRepository()) },
) {
  val state by viewModel.uiState.collectAsStateWithLifecycle()
  MainScreen(state, viewModel::measure, modifier)
}

@Composable
internal fun MainScreen(
  state: MainScreenUiState,
  onMeasure: (Dataset) -> Unit,
  modifier: Modifier = Modifier,
) {
  val result = state.results[state.selected]
  var showCars by rememberSaveable(state.selected) { mutableStateOf(false) }

  Scaffold(modifier = modifier) { insets ->
    Box(Modifier.fillMaxSize().padding(insets), contentAlignment = Alignment.TopCenter) {
      LazyColumn(
        modifier = Modifier.widthIn(max = 620.dp).fillMaxWidth(),
        contentPadding = PaddingValues(start = 20.dp, top = 16.dp, end = 20.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
      ) {
        item { ScreenHeader() }
        item { SourceButtons(state.selected, state.isRunning, onMeasure) }

        if (state.isRunning) {
          item {
            Surface(color = MaterialTheme.colorScheme.surfaceContainer, shape = RoundedCornerShape(16.dp)) {
              Row(
                Modifier.fillMaxWidth().padding(16.dp),
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalAlignment = Alignment.CenterVertically,
              ) {
                CircularProgressIndicator(modifier = Modifier.size(22.dp), strokeWidth = 2.dp)
                Text(stringResource(
                  if (state.selected == Dataset.VEGA_CARS) R.string.measuring_network else R.string.measuring_local,
                ))
              }
            }
          }
        }
        state.error?.let { error ->
          item {
            Surface(color = MaterialTheme.colorScheme.errorContainer, shape = RoundedCornerShape(16.dp)) {
              Text(stringResource(R.string.measure_error, error), Modifier.fillMaxWidth().padding(16.dp))
            }
          }
        }
        if (!state.isRunning && result == null && state.error == null) {
          item {
            Surface(color = MaterialTheme.colorScheme.surfaceContainerLow, shape = RoundedCornerShape(20.dp)) {
              Text(
                stringResource(R.string.choose_source),
                Modifier.fillMaxWidth().padding(20.dp),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
              )
            }
          }
        }
        if (!state.isRunning && result != null) {
          item { ResultCard(checkNotNull(state.selected), result) }
        }

        val previousResults = state.results.filterKeys { it != state.selected }
        if (previousResults.isNotEmpty()) {
          item {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
              Text(stringResource(R.string.previous_results), style = MaterialTheme.typography.titleSmall)
              Dataset.entries.forEach { dataset ->
                previousResults[dataset]?.let { previous ->
                  val fastest = previous.times.first()
                  Row(
                    Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                  ) {
                    Text(dataset.label(), style = MaterialTheme.typography.bodyMedium)
                    Text(
                      stringResource(R.string.fastest_result, fastest.parser, timeLabel(fastest.medianNanos)),
                      style = MaterialTheme.typography.bodyMedium,
                      color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                  }
                }
              }
            }
          }
        }

        if (!state.isRunning && result != null && result.cars.isNotEmpty()) {
          item {
            TextButton(onClick = { showCars = !showCars }) {
              Text(stringResource(if (showCars) R.string.hide_cars else R.string.show_cars))
            }
          }
          if (showCars) {
            items(result.cars) { car -> Text(car, style = MaterialTheme.typography.bodyMedium) }
          }
        }
      }
    }
  }
}

@Composable
private fun ScreenHeader() {
  Surface(
    modifier = Modifier.fillMaxWidth(),
    color = Color(0xFF103041),
    contentColor = Color(0xFFF3F9FC),
    shape = RoundedCornerShape(24.dp),
  ) {
    Column(Modifier.padding(22.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
      Text(
        stringResource(R.string.app_name),
        color = Color(0xFF9DD9EE),
        style = MaterialTheme.typography.labelLarge,
      )
      Text(
        stringResource(R.string.parse_lab),
        style = MaterialTheme.typography.headlineMedium,
        fontWeight = FontWeight.SemiBold,
      )
      Text(stringResource(R.string.parse_lab_description), style = MaterialTheme.typography.bodyMedium)
    }
  }
}

@Composable
private fun SourceButtons(selected: Dataset?, isRunning: Boolean, onMeasure: (Dataset) -> Unit) {
  Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
    Text(stringResource(R.string.catalog_sources), style = MaterialTheme.typography.titleMedium)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
      Dataset.entries.filter { it.catalogSize != null }.forEach { dataset ->
        SourceButton(dataset, selected, isRunning, onMeasure)
      }
    }
    SourceButton(Dataset.VEGA_CARS, selected, isRunning, onMeasure)
  }
}

@Composable
private fun SourceButton(
  dataset: Dataset,
  selected: Dataset?,
  isRunning: Boolean,
  onMeasure: (Dataset) -> Unit,
) {
  val label = if (dataset == Dataset.VEGA_CARS) stringResource(R.string.vega_source_button) else dataset.label()
  val padding = PaddingValues(horizontal = 13.dp, vertical = 8.dp)
  if (selected == dataset) {
    Button(onClick = { onMeasure(dataset) }, enabled = !isRunning, contentPadding = padding) {
      Text(label)
    }
  } else {
    OutlinedButton(onClick = { onMeasure(dataset) }, enabled = !isRunning, contentPadding = padding) {
      Text(label)
    }
  }
}

@Composable
private fun ResultCard(dataset: Dataset, result: ParseResult) {
  Surface(
    modifier = Modifier.fillMaxWidth(),
    color = MaterialTheme.colorScheme.surfaceContainerLow,
    shape = RoundedCornerShape(20.dp),
  ) {
    Column(Modifier.padding(18.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
      Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(
          stringResource(R.string.parse_time_title, dataset.label()),
          style = MaterialTheme.typography.titleLarge,
          fontWeight = FontWeight.SemiBold,
        )
        Text(
          stringResource(R.string.payload_summary, result.byteCount, result.recordCount),
          style = MaterialTheme.typography.bodyMedium,
          color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
      }
      val slowest = result.times.maxOf { it.medianNanos }.coerceAtLeast(1)
      result.times.forEachIndexed { index, time ->
        TimingRow(time, (time.medianNanos.toDouble() / slowest).toFloat(), index == 0)
      }
      Text(
        stringResource(R.string.measurement_note, MEASURED_RUNS),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
      )
    }
  }
}

@Composable
private fun TimingRow(time: ParserTime, fraction: Float, fastest: Boolean) {
  val barColor = if (fastest) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline
  Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
    Row(
      Modifier.fillMaxWidth(),
      horizontalArrangement = Arrangement.spacedBy(8.dp),
      verticalAlignment = Alignment.CenterVertically,
    ) {
      Text(
        text = time.parser,
        modifier = Modifier.weight(1f),
        style = MaterialTheme.typography.bodyMedium,
        fontWeight = if (fastest) FontWeight.Bold else FontWeight.Medium,
      )
      Text(
        text = timeLabel(time.medianNanos),
        style = MaterialTheme.typography.bodyMedium,
        fontFamily = FontFamily.Monospace,
        fontWeight = FontWeight.SemiBold,
      )
    }
    Box(
      Modifier.fillMaxWidth().height(5.dp).clip(CircleShape)
        .background(MaterialTheme.colorScheme.surfaceContainerHighest),
    ) {
      Box(Modifier.fillMaxWidth(fraction).height(5.dp).clip(CircleShape).background(barColor))
    }
  }
}

@Composable
private fun Dataset.label(): String = catalogSize?.label ?: stringResource(R.string.vega_source)

@Composable
private fun timeLabel(nanos: Long): String =
  if (nanos < 1_000_000) stringResource(R.string.time_microseconds, nanos / 1_000.0)
  else stringResource(R.string.time_milliseconds, nanos / 1_000_000.0)

@Preview(showBackground = true, showSystemUi = true)
@Composable
private fun MainScreenPreview() {
  YalinSampleTheme {
    val dataset = Dataset.CATALOG_256KB
    MainScreen(
      state = MainScreenUiState(
        selected = dataset,
        results = mapOf(
          dataset to ParseResult(
            268_512,
            1_465,
            listOf(
              ParserTime("Yalın", 4_200_000),
              ParserTime("fastjson2", 5_500_000),
              ParserTime("Gson", 11_100_000),
              ParserTime("kotlinx.serialization", 13_000_000),
              ParserTime("Moshi", 16_000_000),
            ),
          ),
        ),
      ),
      onMeasure = {},
    )
  }
}
